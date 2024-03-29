use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

#[cfg(feature = "log")]
use log::trace;

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
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            
            if let Some(thread) = worker.thread.take() {
                #[cfg(feature = "log")]
                trace!("Shutting down worker {}", worker._id);
                thread.join().unwrap();
            }
        }
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
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    #[cfg(feature = "log")]
                    trace!("Worker {id} got a job; executing");
                    job();
                }
                Err(_) => {
                    #[cfg(feature = "log")]
                    trace!("Worker {id} disconnected; shutting down");
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