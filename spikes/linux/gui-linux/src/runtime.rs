//! The runtime on the realization's own event loop: timers on a monotonic clock, tasks polled
//! when their wakers fire, and an inbox other threads post to (the settings portal's thread,
//! a task's waker), woken through a pipe the loop polls beside the display's socket.

use std::{
  cell::{Cell, RefCell},
  cmp::Reverse,
  collections::{BinaryHeap, HashMap, HashSet},
  os::fd::RawFd,
  sync::{Arc, Mutex},
  task::{Context, Poll, Wake, Waker},
  time::{Duration, Instant},
};

use skene_foundation::seam::runtime::{Cancel, Task};

/// What another thread posts to the loop.
#[derive(Debug)]
pub enum Message {
  /// The settings portal changed a setting: its namespace, key and value.
  Setting(String, String, crate::environment::SettingValue),
  /// A task's waker fired.
  Wake,
  /// An input method on X11 said something.
  Ime(crate::ime::ImeEvent),
  /// The accessibility layer asked something of the window.
  Access(crate::access::Request),
  /// The GPU device the window and its popup paint with was lost: they paint again, on the
  /// CPU.
  #[cfg(feature = "gpu")]
  DeviceLost,
}

/// A queue the loop drains, woken through a pipe.
#[derive(Clone)]
pub struct Inbox {
  queue: Arc<Mutex<Vec<Message>>>,
  ready: Arc<Mutex<Vec<u64>>>,
  /// The pipe's read end, for the loop to poll.
  pub read: RawFd,
  write: RawFd,
}

impl Inbox {
  fn new() -> Self {
    let mut fds = [0; 2];

    // SAFETY: `fds` is two integers for pipe2 to fill.
    let result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };

    assert_eq!(result, 0, "the loop's wake pipe");

    Self {
      queue: Arc::default(),
      ready: Arc::default(),
      read: fds[0],
      write: fds[1],
    }
  }

  fn wake(&self) {
    // SAFETY: writes one byte from a live buffer to the pipe's write end.
    unsafe {
      libc::write(self.write, [1u8].as_ptr().cast(), 1);
    }
  }

  /// Queues `message` and wakes the loop.
  pub fn send(&self, message: Message) {
    self
      .queue
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push(message);
    self.wake();
  }

  /// Takes every queued message, emptying the pipe.
  pub fn drain(&self) -> Vec<Message> {
    let mut buffer = [0u8; 64];

    // SAFETY: reads into a live buffer from the non-blocking read end until it is empty.
    while unsafe { libc::read(self.read, buffer.as_mut_ptr().cast(), buffer.len()) } > 0 {}

    std::mem::take(
      &mut *self
        .queue
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
  }
}

struct TaskWaker {
  id: u64,
  inbox: Inbox,
}

impl Wake for TaskWaker {
  fn wake(self: Arc<Self>) {
    self.wake_by_ref();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self
      .inbox
      .ready
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push(self.id);
    self.inbox.wake();
  }
}

#[derive(Default)]
struct Runtime {
  timers: BinaryHeap<Reverse<(Instant, u64)>>,
  actions: HashMap<u64, Box<dyn FnOnce()>>,
  tasks: HashMap<u64, Task>,
  cancelled: HashSet<u64>,
}

thread_local! {
  static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::default());
  static INBOX: Inbox = Inbox::new();
  static NEXT: Cell<u64> = const { Cell::new(1) };
}

fn next_id() -> u64 {
  NEXT.with(|next| {
    let id = next.get();

    next.set(id + 1);

    id
  })
}

/// This thread's inbox.
pub fn inbox() -> Inbox {
  INBOX.with(Clone::clone)
}

/// Runs `action` once after `delay`.
pub fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Cancel {
  let id = next_id();

  RUNTIME.with(|runtime| {
    let mut runtime = runtime.borrow_mut();

    runtime.timers.push(Reverse((Instant::now() + delay, id)));
    runtime.actions.insert(id, action);
  });
  INBOX.with(Inbox::wake);

  Cancel(Some(Box::new(move || {
    RUNTIME.with(|runtime| {
      runtime.borrow_mut().actions.remove(&id);
    });
  })))
}

/// Polls `task` on the loop until it finishes.
pub fn spawn(task: Task) -> Cancel {
  let id = next_id();

  RUNTIME.with(|runtime| {
    runtime.borrow_mut().tasks.insert(id, task);
  });
  INBOX.with(|inbox| {
    inbox
      .ready
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push(id);
    inbox.wake();
  });

  Cancel(Some(Box::new(move || {
    RUNTIME.with(|runtime| {
      let mut runtime = runtime.borrow_mut();

      if runtime.tasks.remove(&id).is_none() {
        runtime.cancelled.insert(id);
      }
    });
  })))
}

/// How long the loop may sleep before a timer is due; `None` where none is set.
pub fn timeout() -> Option<Duration> {
  RUNTIME.with(|runtime| {
    let mut runtime = runtime.borrow_mut();

    loop {
      let Reverse((at, id)) = *runtime.timers.peek()?;

      if runtime.actions.contains_key(&id) {
        return Some(at.saturating_duration_since(Instant::now()));
      }

      runtime.timers.pop();
    }
  })
}

/// Fires the timers that are due and polls the tasks whose wakers fired: what the loop runs
/// after each wake.
pub fn run_due() {
  let now = Instant::now();

  loop {
    let due = RUNTIME.with(|runtime| {
      let mut runtime = runtime.borrow_mut();
      let Reverse((at, id)) = *runtime.timers.peek()?;

      if at > now {
        return None;
      }

      runtime.timers.pop();

      Some(runtime.actions.remove(&id))
    });

    match due {
      Some(Some(action)) => action(),
      Some(None) => {}
      None => break,
    }
  }

  let inbox = inbox();

  loop {
    let ready: Vec<u64> = std::mem::take(
      &mut *inbox
        .ready
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner),
    );

    if ready.is_empty() {
      break;
    }

    for id in ready {
      let Some(mut task) = RUNTIME.with(|runtime| runtime.borrow_mut().tasks.remove(&id)) else {
        continue;
      };
      let waker = Waker::from(Arc::new(TaskWaker {
        id,
        inbox: inbox.clone(),
      }));
      let mut context = Context::from_waker(&waker);

      if task.as_mut().poll(&mut context) == Poll::Pending {
        RUNTIME.with(|runtime| {
          let mut runtime = runtime.borrow_mut();

          if !runtime.cancelled.remove(&id) {
            runtime.tasks.insert(id, task);
          }
        });
      } else {
        RUNTIME.with(|runtime| runtime.borrow_mut().cancelled.remove(&id));
      }
    }
  }
}
