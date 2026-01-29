//! Job scheduler for managing scheduled handler execution.
//!
//! The scheduler maintains a collection of jobs and provides methods to:
//! - Add new jobs
//! - Cancel existing jobs
//! - Query the next deadline for poll timeout calculation
//! - Retrieve and process due jobs

use std::time::{Duration, Instant};

use crate::instance::InstanceId;
use crate::job::{JobId, Schedule, ScheduledJob};

// =============================================================================
// JobScheduler
// =============================================================================

/// Manages scheduled jobs for the runtime.
///
/// Jobs are stored in a simple Vec and processed linearly. For most use cases
/// (dozens of jobs), this is efficient enough. If needed, this could be
/// optimized to use a priority queue.
pub(crate) struct JobScheduler {
    jobs: Vec<ScheduledJob>,
}

impl JobScheduler {
    /// Create a new empty scheduler.
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    /// Add a job to the scheduler.
    ///
    /// Returns the job's ID for later cancellation.
    pub fn add(&mut self, job: ScheduledJob) -> JobId {
        let id = job.id;
        log::debug!(
            "[scheduler] Adding job {:?}, next_run in {:?}",
            id,
            job.next_run().saturating_duration_since(Instant::now())
        );
        self.jobs.push(job);
        id
    }

    /// Cancel a job by ID.
    ///
    /// Returns true if the job was found and cancelled.
    pub fn cancel(&mut self, id: JobId) -> bool {
        let len_before = self.jobs.len();
        self.jobs.retain(|job| job.id != id);
        let cancelled = self.jobs.len() < len_before;
        if cancelled {
            log::debug!("[scheduler] Cancelled job {:?}", id);
        }
        cancelled
    }

    /// Cancel all jobs associated with an instance.
    ///
    /// Called when an app instance is closed to clean up its scheduled jobs.
    pub fn cancel_for_instance(&mut self, instance_id: InstanceId) {
        let len_before = self.jobs.len();
        self.jobs
            .retain(|job| job.source_instance != Some(instance_id));
        let cancelled = len_before - self.jobs.len();
        if cancelled > 0 {
            log::debug!(
                "[scheduler] Cancelled {} jobs for instance {:?}",
                cancelled,
                instance_id
            );
        }
    }

    /// Get the next deadline (earliest job due time).
    ///
    /// Returns None if no jobs are scheduled.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.jobs.iter().map(|job| job.next_run()).min()
    }

    /// Get the duration until the next job is due.
    ///
    /// Returns None if no jobs are scheduled.
    /// Returns Some(Duration::ZERO) if a job is already due.
    pub fn time_until_next(&self) -> Option<Duration> {
        self.next_deadline()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    /// Take all jobs that are due to run.
    ///
    /// Due jobs are removed from the scheduler. For interval jobs, they should
    /// be re-added with an updated next_run time after execution.
    ///
    /// Returns the jobs that should be executed.
    pub fn take_due(&mut self, now: Instant) -> Vec<ScheduledJob> {
        let mut due = Vec::new();
        let mut remaining = Vec::new();

        for job in self.jobs.drain(..) {
            if job.is_due(now) {
                log::debug!("[scheduler] Job {:?} is due", job.id);
                due.push(job);
            } else {
                remaining.push(job);
            }
        }

        self.jobs = remaining;
        due
    }

    /// Re-add an interval job with updated next_run time.
    ///
    /// Call this after executing an interval job to schedule its next run.
    pub fn reschedule_interval(&mut self, job: ScheduledJob) {
        if let Schedule::Every { interval, .. } = job.schedule {
            let next_run = Instant::now() + interval;
            log::debug!(
                "[scheduler] Rescheduling interval job {:?}, next_run in {:?}",
                job.id,
                interval
            );
            self.jobs.push(ScheduledJob {
                id: job.id,
                schedule: Schedule::Every { interval, next_run },
                handler: job.handler,
                source_instance: job.source_instance,
            });
        }
    }

    /// Get the number of scheduled jobs.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Check if there are no scheduled jobs.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}
