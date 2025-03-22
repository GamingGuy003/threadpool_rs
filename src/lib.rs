use std::{
    sync::{atomic::AtomicUsize, mpsc, Arc, Mutex},
    thread,
};

/// Threadpool struct storing workers, their sender and how many jobs are still to be completed
#[allow(dead_code)]
pub struct Threadpool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Option<Job>>,
    pending: Arc<AtomicUsize>,
}

/// Job type for workers
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Threadpool logic
impl Threadpool {
    /// Creates a new threadpool with `size` as the amount of parallel workers
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let pending = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(size);

        // Give each worker a receiver and access to the number of still remaining jobs
        for id in 0..size {
            let receiver_c = receiver.clone();
            let pending_c = pending.clone();
            workers.push(Worker::new(id, receiver_c, pending_c));
        }

        Self {
            workers,
            sender,
            pending,
        }
    }

    /// Adds jobs to the queue to be executed by workers
    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // Send job to queue
        self.sender
            .send(Some(Box::new(f)))
            .expect("Failed to send job");
        // Increment pending counter
        self.pending
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn join(&mut self) {
        while self.pending.load(std::sync::atomic::Ordering::SeqCst) > 0 {}
        /*
        // If there are still pending jobs, yield cpu
        while self.pending.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            std::thread::yield_now();
        }

        // Send shutdown signal to all running threads
        for _ in &self.workers {
            self.sender
                .send(None)
                .expect("Failed to send shutdown signal to thread");
        }

        // Join worker threads
        for worker in &mut self.workers {
            worker
                .thread
                .take()
                .expect("Thread already joined")
                .join()
                .expect("Worker failed to join");
        }
        */
    }
}

/// Worker thread
#[allow(dead_code)]
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

/// Worker logic
impl Worker {
    pub fn new(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Option<Job>>>>,
        pending: Arc<AtomicUsize>,
    ) -> Self {
        let thread = thread::spawn(move || loop {
            match receiver
                .lock()
                .expect("Failed to lock receiver channel")
                .recv()
            {
                // Received a job to run
                Ok(Some(job)) => {
                    job();
                    pending.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    #[cfg(feature = "log")]
                    log::debug!(
                        "Worker {id} finished. {} Tasks remain.",
                        pending.load(std::sync::atomic::Ordering::SeqCst)
                    );
                }
                // Received an early abort
                Ok(None) => {
                    #[cfg(feature = "log")]
                    log::trace!("Received abort. Stopping thread {id}");
                    break;
                }
                // Received an error
                Err(_) => {
                    #[cfg(feature = "log")]
                    log::trace!("Channel closed. Stopping thread {id}");
                    break;
                }
            };
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
