//! Scheduled job types for deferred and recurring handler execution.
//!
//! Jobs allow apps and systems to schedule handler calls to run:
//! - After a delay (one-time)
//! - At fixed intervals (recurring)
//!
//! # Example
//!
//! ```ignore
//! #[handler]
//! async fn on_start(&self, gx: &GlobalContext) {
//!     // Run once after 5 seconds
//!     gx.schedule_after(Duration::from_secs(5), self.refresh_data());
//!     
//!     // Run every 30 seconds
//!     let job_id = gx.schedule_every(Duration::from_secs(30), self.poll_api());
//!     self.poll_job.set(Some(job_id));
//! }
//!
//! #[handler]
//! async fn stop_polling(&self, gx: &GlobalContext) {
//!     if let Some(id) = self.poll_job.get() {
//!         gx.cancel_job(id);
//!     }
//! }
//! ```

use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::handler_context::Handler;
use crate::instance::InstanceId;

// =============================================================================
// JobId
// =============================================================================

/// Unique identifier for a scheduled job.
///
/// Used to cancel jobs before they execute.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct JobId(Uuid);

impl JobId {
    /// Create a new unique job ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Schedule
// =============================================================================

/// The schedule type for a job.
#[derive(Clone, Debug)]
pub enum Schedule {
    /// Run once after the specified time, then remove.
    After {
        /// When the job should run.
        due_at: Instant,
    },
    /// Run repeatedly at fixed intervals.
    Every {
        /// The interval between runs.
        interval: Duration,
        /// When the next run should occur.
        next_run: Instant,
    },
}

impl Schedule {
    /// Create a one-time schedule that fires after the given delay.
    pub fn after(delay: Duration) -> Self {
        Self::After {
            due_at: Instant::now() + delay,
        }
    }

    /// Create a recurring schedule that fires at the given interval.
    ///
    /// The first execution is immediate (next_run = now).
    pub fn every(interval: Duration) -> Self {
        Self::Every {
            interval,
            next_run: Instant::now(),
        }
    }

    /// Create a recurring schedule with an initial delay before the first execution.
    pub fn every_after(interval: Duration, initial_delay: Duration) -> Self {
        Self::Every {
            interval,
            next_run: Instant::now() + initial_delay,
        }
    }

    /// Get the next run time for this schedule.
    pub fn next_run(&self) -> Instant {
        match self {
            Self::After { due_at } => *due_at,
            Self::Every { next_run, .. } => *next_run,
        }
    }

    /// Check if this job is due to run.
    pub fn is_due(&self, now: Instant) -> bool {
        self.next_run() <= now
    }
}

// =============================================================================
// ScheduledJob
// =============================================================================

/// A scheduled job awaiting execution.
///
/// This is an internal type used by the scheduler. Users should use
/// `GlobalContext::schedule_after` and `GlobalContext::schedule_every` instead
/// of constructing jobs directly.
#[doc(hidden)]
pub struct ScheduledJob {
    /// Unique identifier for this job.
    pub id: JobId,
    /// The schedule (when/how often to run).
    pub schedule: Schedule,
    /// The handler to execute.
    pub handler: Handler,
    /// Source instance ID (for app jobs). None for system jobs.
    pub source_instance: Option<InstanceId>,
}

impl ScheduledJob {
    /// Create a new scheduled job.
    pub fn new(schedule: Schedule, handler: Handler, source_instance: Option<InstanceId>) -> Self {
        Self {
            id: JobId::new(),
            schedule,
            handler,
            source_instance,
        }
    }

    /// Check if this job is due to run.
    pub fn is_due(&self, now: Instant) -> bool {
        self.schedule.is_due(now)
    }

    /// Get the next run time.
    pub fn next_run(&self) -> Instant {
        self.schedule.next_run()
    }
}
