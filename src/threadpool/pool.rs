use std::{
    sync::{atomic::AtomicUsize, mpsc, Arc, Mutex},
    thread,
};

pub struct Threadpool {
    _workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
    pending: Arc<AtomicUsize>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl Threadpool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let pending = Arc::new(AtomicUsize::new(0));

        let mut _workers = Vec::with_capacity(size);

        for id in 0..size {
            let receiver_c = receiver.clone();
            let pending_c = pending.clone();
            _workers.push(Worker::new(id, receiver_c, pending_c));
        }

        Self {
            _workers,
            sender,
            pending,
        }
    }

    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).expect("Failed to send job");
        self.pending
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn join(&self) {
        while self.pending.load(std::sync::atomic::Ordering::SeqCst) > 0 {}
    }
}

struct Worker {
    _id: usize,
    _thread: thread::JoinHandle<()>,
}

impl Worker {
    pub fn new(
        _id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
        pending: Arc<AtomicUsize>,
    ) -> Self {
        let _thread = thread::spawn(move || loop {
            let job = match receiver
                .lock()
                .expect("Failed to lock receive channel")
                .recv()
            {
                Ok(job) => job,
                Err(_) => {
                    #[cfg(feature = "log")]
                    log::trace!("Channel closed. Stopping thread");
                    break;
                }
            };

            job();
            pending.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            #[cfg(feature = "log")]
            log::debug!(
                "Worker {} finished. {} Tasks remain.",
                _id,
                pending.load(std::sync::atomic::Ordering::SeqCst)
            );
        });

        Worker { _id, _thread }
    }
}
