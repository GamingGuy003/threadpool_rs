use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> Result<ThreadPool, std::io::Error> {
        if size == 0 { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Cannot create pool with zero threads")) }

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        Ok(ThreadPool {
            workers,
            sender: Some(sender),
        })
    }

    pub fn execute<F>(&self, f: F) where F: FnOnce() + Send + 'static, {
        let job = Box::new(f);
        self.sender.as_ref().expect("Failed to get sender").send(job).expect("Failed to send job to thread");
    }

    pub fn join(&mut self) {
        for worker in &mut self.workers {
            println!("Joining worker {}", worker._id);
            if let Some(thread) = worker.thread.take() {
                #[cfg(feature = "log")]
                log::trace!("Joining worker {}", worker._id);
                thread.join().expect("Failed to join thread");
            }
        }
        println!("Done joining");
    } 
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.join();
        drop(self.sender.take());

    }
}

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            // do not move this into the match statement or the threads will not run in parallel for some reason??
            let message = receiver.lock().expect("Failed to lock worker receiver").recv();

            match message {
                Ok(job) => {
                    #[cfg(feature = "log")]
                    log::trace!("Worker {id} got a job; executing");
                    job();
                }
                Err(_) => {
                    #[cfg(feature = "log")]
                    log::trace!("Worker {id} disconnected; shutting down");
                    break;
                }
            }
        });

        Worker {
            _id: id,
            thread: Some(thread),
        }
    }
}
