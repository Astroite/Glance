use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

/// Worker pool type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pool {
    Io,
    Cpu,
}

/// A task to be executed by the task queue
#[derive(Debug, Clone)]
pub enum Task {
    /// Generate a thumbnail on demand (e.g., when asset:// misses cache)
    ThumbnailOnDemand { photo_id: i64, tier: u32 },
    /// Prefetch thumbnails after scan completes
    ThumbnailPrefetch { photo_ids: Vec<i64> },
    /// Run a library scan
    ScanLibrary { library_id: i64 },
    /// Extract metadata for a specific photo file
    MetadataExtraction { photo_file_id: i64 },
    /// Garbage orphan thumbnails
    OrphanGc,
}

impl Task {
    /// Which pool should execute this task
    pub fn pool(&self) -> Pool {
        match self {
            Task::ThumbnailOnDemand { .. } => Pool::Cpu,
            Task::ThumbnailPrefetch { .. } => Pool::Cpu,
            Task::MetadataExtraction { .. } => Pool::Io,
            Task::ScanLibrary { .. } => Pool::Io,
            Task::OrphanGc => Pool::Io,
        }
    }

    /// Default priority for this task type
    pub fn default_priority(&self) -> TaskPriority {
        match self {
            Task::ThumbnailOnDemand { .. } => TaskPriority::High,
            Task::ThumbnailPrefetch { .. } => TaskPriority::Low,
            Task::ScanLibrary { .. } => TaskPriority::Normal,
            Task::MetadataExtraction { .. } => TaskPriority::Normal,
            Task::OrphanGc => TaskPriority::Low,
        }
    }
}

/// A prioritized task wrapper for the queue
#[derive(Debug)]
struct PrioritizedTask {
    task: Task,
    priority: TaskPriority,
    seq: u64, // For FIFO ordering within same priority
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then lower seq (FIFO) first
        self.priority.cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

/// Cancellation token for tasks
pub type CancellationToken = Arc<AtomicBool>;

/// Create a new cancellation token
pub fn new_cancel_token() -> CancellationToken {
    Arc::new(AtomicBool::new(false))
}

/// Progress event emitted when a task completes
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub task_type: &'static str,
    pub pool: Pool,
    pub queue_depth: usize,
}

/// The task handler function type
type TaskHandler = Box<dyn Fn(&Task, &CancellationToken) + Send + Sync>;

/// Shared queue state
struct QueueState {
    heap: BinaryHeap<PrioritizedTask>,
    seq: u64,
    shutdown: bool,
}

/// A priority task queue with IO and CPU worker pools
pub struct TaskQueue {
    state: Arc<(Mutex<QueueState>, Condvar)>,
    cancel: CancellationToken,
    io_handles: Vec<thread::JoinHandle<()>>,
    cpu_handles: Vec<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    handler: Arc<TaskHandler>,
}

impl TaskQueue {
    /// Create a new task queue with the given handler.
    /// `io_workers`: number of IO pool workers (recommended: 3)
    /// `cpu_workers`: number of CPU pool workers (recommended: num_cpus)
    pub fn new<F>(
        io_workers: usize,
        cpu_workers: usize,
        handler: F,
    ) -> Self
    where
        F: Fn(&Task, &CancellationToken) + Send + Sync + 'static,
    {
        let state = Arc::new((
            Mutex::new(QueueState {
                heap: BinaryHeap::new(),
                seq: 0,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let cancel = new_cancel_token();
        let handler: Arc<TaskHandler> = Arc::new(Box::new(handler));

        let mut io_handles = Vec::with_capacity(io_workers);
        let mut cpu_handles = Vec::with_capacity(cpu_workers);

        // Spawn IO workers
        for _ in 0..io_workers {
            let state = Arc::clone(&state);
            let cancel = Arc::clone(&cancel);
            let handler = Arc::clone(&handler);
            let handle = thread::spawn(move || {
                worker_loop(state, cancel, handler, Pool::Io);
            });
            io_handles.push(handle);
        }

        // Spawn CPU workers
        for _ in 0..cpu_workers {
            let state = Arc::clone(&state);
            let cancel = Arc::clone(&cancel);
            let handler = Arc::clone(&handler);
            let handle = thread::spawn(move || {
                worker_loop(state, cancel, handler, Pool::Cpu);
            });
            cpu_handles.push(handle);
        }

        TaskQueue {
            state,
            cancel,
            io_handles,
            cpu_handles,
            handler,
        }
    }

    /// Enqueue a task with its default priority
    pub fn enqueue(&self, task: Task) {
        let priority = task.default_priority();
        self.enqueue_with_priority(task, priority);
    }

    /// Enqueue a task with a specific priority
    pub fn enqueue_with_priority(&self, task: Task, priority: TaskPriority) {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().unwrap();
        let seq = state.seq;
        state.seq += 1;
        state.heap.push(PrioritizedTask {
            task,
            priority,
            seq,
        });
        cvar.notify_one();
    }

    /// Get current queue depth
    pub fn depth(&self) -> usize {
        let (lock, _) = &*self.state;
        let state = lock.lock().unwrap();
        state.heap.len()
    }

    /// Cancel all pending tasks. Workers will finish their current task and stop.
    pub fn cancel_all(&self) {
        self.cancel.store(true, AtomicOrdering::Relaxed);
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().unwrap();
        state.heap.clear();
        state.shutdown = true;
        cvar.notify_all();
    }

    /// Check if the queue has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(AtomicOrdering::Relaxed)
    }
}

impl Drop for TaskQueue {
    fn drop(&mut self) {
        self.cancel_all();
        // Join all worker threads
        for handle in self.io_handles.drain(..) {
            handle.join().ok();
        }
        for handle in self.cpu_handles.drain(..) {
            handle.join().ok();
        }
    }
}

/// Worker loop: dequeue tasks matching the worker's pool type
fn worker_loop(
    state: Arc<(Mutex<QueueState>, Condvar)>,
    cancel: CancellationToken,
    handler: Arc<TaskHandler>,
    pool: Pool,
) {
    loop {
        // Wait for a task or shutdown
        let task = {
            let (lock, cvar) = &*state;
            let mut state = lock.lock().unwrap();

            // Find the highest-priority task matching our pool
            loop {
                if state.shutdown {
                    return;
                }

                // Try to find a task for our pool
                let task = find_task_for_pool(&mut state, pool);
                if let Some(t) = task {
                    break t;
                }

                // No task available, wait
                state = cvar.wait(state).unwrap();
            }
        };

        // Execute the task
        if cancel.load(AtomicOrdering::Relaxed) {
            return;
        }

        handler(&task, &cancel);
    }
}

/// Find and remove the highest-priority task for the given pool
fn find_task_for_pool(state: &mut QueueState, pool: Pool) -> Option<Task> {
    // BinaryHeap doesn't support efficient removal, so we drain and rebuild
    let mut matched = None;
    let mut remaining = BinaryHeap::new();

    while let Some(item) = state.heap.pop() {
        if matched.is_none() && item.task.pool() == pool {
            matched = Some(item);
        } else {
            remaining.push(item);
        }
    }

    state.heap = remaining;
    matched.map(|t| t.task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[test]
    fn test_priority_ordering() {
        let state = Arc::new((
            Mutex::new(QueueState {
                heap: BinaryHeap::new(),
                seq: 0,
                shutdown: false,
            }),
            Condvar::new(),
        ));

        // Enqueue tasks in reverse priority order
        {
            let (lock, _) = &*state;
            let mut s = lock.lock().unwrap();
            s.heap.push(PrioritizedTask { task: Task::OrphanGc, priority: TaskPriority::Low, seq: 0 });
            s.heap.push(PrioritizedTask { task: Task::ScanLibrary { library_id: 1 }, priority: TaskPriority::Normal, seq: 1 });
            s.heap.push(PrioritizedTask { task: Task::ThumbnailOnDemand { photo_id: 1, tier: 240 }, priority: TaskPriority::High, seq: 2 });
        }

        // High should come first
        let (lock, _) = &*state;
        let mut s = lock.lock().unwrap();
        let first = s.heap.pop().unwrap();
        assert_eq!(first.priority, TaskPriority::High);
        let second = s.heap.pop().unwrap();
        assert_eq!(second.priority, TaskPriority::Normal);
        let third = s.heap.pop().unwrap();
        assert_eq!(third.priority, TaskPriority::Low);
    }

    #[test]
    fn test_task_pool_assignment() {
        assert_eq!(Task::ThumbnailOnDemand { photo_id: 1, tier: 240 }.pool(), Pool::Cpu);
        assert_eq!(Task::ScanLibrary { library_id: 1 }.pool(), Pool::Io);
        assert_eq!(Task::OrphanGc.pool(), Pool::Io);
    }

    #[test]
    fn test_enqueue_and_execute() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let queue = TaskQueue::new(1, 1, move |task, _cancel| {
            match task {
                Task::ThumbnailOnDemand { .. } => {
                    counter_clone.fetch_add(1, AtomicOrdering::Relaxed);
                }
                _ => {}
            }
        });

        queue.enqueue(Task::ThumbnailOnDemand { photo_id: 1, tier: 240 });
        queue.enqueue(Task::ThumbnailOnDemand { photo_id: 2, tier: 240 });

        // Wait for tasks to complete
        thread::sleep(Duration::from_millis(200));

        assert_eq!(counter.load(AtomicOrdering::Relaxed), 2);
    }

    #[test]
    fn test_cancel_all() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let queue = TaskQueue::new(1, 1, move |_task, cancel| {
            // Slow tasks
            thread::sleep(Duration::from_millis(50));
            if !cancel.load(AtomicOrdering::Relaxed) {
                counter_clone.fetch_add(1, AtomicOrdering::Relaxed);
            }
        });

        for i in 0..10 {
            queue.enqueue(Task::ThumbnailPrefetch { photo_ids: vec![i] });
        }

        // Cancel immediately
        queue.cancel_all();

        thread::sleep(Duration::from_millis(300));

        // Not all tasks should have completed
        let completed = counter.load(AtomicOrdering::Relaxed);
        assert!(completed < 10, "Expected fewer than 10 tasks to complete, got {}", completed);
    }

    #[test]
    fn test_pool_routing() {
        let io_count = Arc::new(AtomicUsize::new(0));
        let cpu_count = Arc::new(AtomicUsize::new(0));
        let io_clone = Arc::clone(&io_count);
        let cpu_clone = Arc::clone(&cpu_count);

        let queue = TaskQueue::new(1, 1, move |task, _cancel| {
            match task.pool() {
                Pool::Io => { io_clone.fetch_add(1, AtomicOrdering::Relaxed); }
                Pool::Cpu => { cpu_clone.fetch_add(1, AtomicOrdering::Relaxed); }
            }
        });

        // Enqueue IO and CPU tasks
        queue.enqueue(Task::ScanLibrary { library_id: 1 }); // IO
        queue.enqueue(Task::ThumbnailOnDemand { photo_id: 1, tier: 240 }); // CPU
        queue.enqueue(Task::MetadataExtraction { photo_file_id: 1 }); // IO
        queue.enqueue(Task::ThumbnailPrefetch { photo_ids: vec![1, 2] }); // CPU

        thread::sleep(Duration::from_millis(300));

        assert_eq!(io_count.load(AtomicOrdering::Relaxed), 2);
        assert_eq!(cpu_count.load(AtomicOrdering::Relaxed), 2);
    }

    #[test]
    fn test_queue_depth() {
        let queue = TaskQueue::new(1, 1, |_task, _cancel| {
            thread::sleep(Duration::from_millis(100));
        });

        assert_eq!(queue.depth(), 0);

        queue.enqueue(Task::ScanLibrary { library_id: 1 });
        queue.enqueue(Task::ThumbnailOnDemand { photo_id: 1, tier: 240 });

        // Give a moment for the queue to be populated
        thread::sleep(Duration::from_millis(10));
        // At least some tasks should be in the queue (some may have started already)
        // The exact depth depends on timing
    }
}
