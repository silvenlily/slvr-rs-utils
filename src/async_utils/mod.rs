use std::cmp::min;
use std::sync::Arc;

use conv::*;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct JoinHandleListSelfCleaning {
    backing: Arc<Mutex<Vec<JoinHandle<()>>>>,
    clearer: Arc<Mutex<Option<JoinHandle<()>>>>,
    check_at: u64,
    check_count: Arc<Mutex<u64>>,
}

impl JoinHandleListSelfCleaning {
    async fn push(&mut self, handle: JoinHandle<()>) {
        let mut backing = self.backing.lock().await;

        backing.push(handle);

        {
            let mut check_count = self.check_count.lock().await;
            if check_count.lt(&self.check_at) {
                return;
            }
            *check_count = 0;
        };

        let mut clearer = self.clearer.lock().await;
        if clearer.is_some() {
            if let Some(c) = &*clearer {
                if !c.is_finished() {
                    return;
                }
            }
        }

        let arc_backing = self.backing.clone();

        *clearer = Some(tokio::spawn(async move {
            let mut join_handles = arc_backing.lock().await;

            let i = 0_usize;
            'clear_old_handles: while i < join_handles.len() {
                if let Some(handle) = join_handles.get(i) {
                    if handle.is_finished() {
                        join_handles.swap_remove(i);
                        if join_handles.len() < 8_usize {
                            break 'clear_old_handles;
                        }
                    }
                } else {
                    break 'clear_old_handles;
                }
            }
        }));
    }
    fn new(starting_capacity: usize) -> Self {

        // the fact that i need to do this much type conversion is fucking ridiculus
        let check_at: u64 = {
            if starting_capacity > 16 {
                let approx_starting_capacity:Result<f64,_> = starting_capacity.approx();
                match approx_starting_capacity {
                    Ok(cap) => {
                        let try_ca:Result<u64,_> = (cap/1.5).approx();
                        match try_ca {
                            Ok(ca) => {
                                ca
                            }
                            Err(_) => {
                                8
                            }
                        }
                    }
                    Err(_) => {
                        8
                    }
                }
            } else {
                8
            }

        };

        return JoinHandleListSelfCleaning {
            backing: Arc::new(Mutex::new(Vec::with_capacity(starting_capacity))),
            clearer: Arc::new(Mutex::new(None)),
            check_at,
            check_count: Arc::new(Mutex::new(0)),
        };
    }
}