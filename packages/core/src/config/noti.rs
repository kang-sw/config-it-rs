//! A naive 'watch' implementation for monitoring updates

use std::{
    sync::{Arc, Weak},
    task::{Poll, Waker},
};

use parking_lot::Mutex;
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct Sender(Arc<Mutex<Inner>>);

#[derive(Debug, Clone)]
pub struct Receiver(usize, Weak<Mutex<Inner>>);

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

impl Sender {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Inner { fence: 1, waiters: Default::default() })))
    }

    pub fn notify(&self) {
        let mut inner = self.0.lock();
        inner.fence = inner.fence.wrapping_add(2); // It never touches the value '0'
        inner.waiters.drain(..).for_each(|x| x.1.wake());
    }

    pub fn receiver(&self, fresh: bool) -> Receiver {
        Receiver(if fresh { 0 } else { self.0.lock().fence }, Arc::downgrade(&self.0))
    }
}

#[derive(Debug)]
struct Inner {
    fence: usize,
    waiters: SmallVec<[(usize, Waker); 4]>,
}

impl Receiver {
    pub fn invalidate(&mut self) {
        self.0 = 0;
    }

    pub fn has_update(&self) -> Option<bool> {
        self.1.upgrade().map(|x| x.lock().fence != self.0)
    }

    pub fn try_recv(&mut self) -> Result<(), TryWaitError> {
        let flag = self.1.upgrade().ok_or(TryWaitError::Closed)?.lock().fence;
        if self.0 != flag {
            self.0 = flag;
            Ok(())
        } else {
            Err(TryWaitError::Empty)
        }
    }

    pub fn recv(&mut self) -> Wait {
        Wait { rx: self, state: WaitState::Created }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TryWaitError {
    #[error("Closed notify channel")]
    Closed,

    #[error("There's no update")]
    Empty,
}

#[derive(thiserror::Error, Debug)]
pub enum WaitError {
    #[error("Closed notify channel")]
    Closed,

    #[error("Expired notify channel")]
    Expired,
}

#[derive(Debug)]
pub struct Wait<'a> {
    rx: &'a mut Receiver,
    state: WaitState,
}

#[derive(Debug, Clone, Copy)]
enum WaitState {
    Created,
    Registered,
    Expired,
}

impl<'a> Wait<'a> {
    fn unregister(&mut self) {
        let id = self.get_id();

        // Must be called with 'unregister-able' state.
        debug_assert!(matches!(self.state, WaitState::Registered));

        // Sender is allowed to be disposed at any time.
        let Some(inner) = self.rx.1.upgrade() else { return };
        let inner = &mut inner.lock().waiters;

        // Remove the waiter from the list.
        if let Some(idx) = inner.iter().position(|x| x.0 == id) {
            inner.swap_remove(idx);
        } else {
            // It's okay if the waiter was not found, as it could be unregistered during this
            // operation.
        }
    }

    fn get_id(&self) -> usize {
        self.rx as *const _ as usize
    }
}

impl<'a> Drop for Wait<'a> {
    fn drop(&mut self) {
        if matches!(self.state, WaitState::Registered) {
            self.unregister();
        }
    }
}

impl<'a> std::future::Future for Wait<'a> {
    type Output = Result<(), WaitError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let id = this.get_id();

        match this.state {
            WaitState::Created => {
                let Some(inner) = this.rx.1.upgrade() else {
                    this.state = WaitState::Expired;
                    return Poll::Ready(Err(WaitError::Closed));
                };

                let mut inner = inner.lock();

                if inner.fence != this.rx.0 {
                    // Fast-path for early wakeup one
                    this.rx.0 = inner.fence;
                    return Poll::Ready(Ok(()));
                }

                inner.waiters.push((id, cx.waker().clone()));
                this.state = WaitState::Registered;

                Poll::Pending
            }

            WaitState::Registered => {
                let Some(inner) = this.rx.1.upgrade() else {
                    this.state = WaitState::Expired;
                    return Poll::Ready(Err(WaitError::Closed));
                };

                let mut inner = inner.lock();
                
                if inner.fence != this.rx.0 {
                    this.rx.0 = inner.fence;
                    this.state = WaitState::Expired;

                    Poll::Ready(Ok(()))
                } else {
                    if inner.waiters.iter().find(|x| x.0 == id).is_none() {
                        // For falsy wakeup, registers itself again
                        // Remove the waiter from the list.
                        inner.waiters.push((id, cx.waker().clone()));
                    } else {
                        // If the waiter was found, it means that the waker was already registered
                        // and it's okay to leave it as it is.
                    }
                    
                    Poll::Pending
                }
            }

            WaitState::Expired => Poll::Ready(Err(WaitError::Expired)),
        }
    }
}
