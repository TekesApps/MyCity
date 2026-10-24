//! Retained ownership for one optional broker registration and its real child.
//!
//! Intended to be included as `normal_pool::startup_owner`, so the child module
//! can use the existing private workspace kind without widening broker APIs.
//! This owner does not schedule, launch helpers, or create a second pool.

use super::{broker::{PendingRegistration, Supervisor}, workspace::Kind};
use std::{path::Path, process::{Child, Command, ExitStatus}};

/// Owns every capability needed to finish startup and retire one process.
/// Secret-bearing broker types intentionally prevent `Debug`, `Clone`, and
/// serialization derives here. Dropping this value is not a retirement proof.
pub struct StartupOwner {
    pending: Option<PendingRegistration>,
    supervisor: Option<Supervisor>,
    child: Option<Child>,
    bind_acknowledged: bool,
    real_exit: Option<ExitStatus>,
}

impl StartupOwner {
    /// Construct local empty state only; does not inspect or contact the broker.
    pub fn new() -> Self {
        Self {
            pending: None,
            supervisor: None,
            child: None,
            bind_acknowledged: false,
            real_exit: None,
        }
    }

    /// True only after a process is waited and any broker revoke/ack succeeds.
    pub fn is_empty(&self) -> bool {
        self.pending.is_none()
            && self.supervisor.is_none()
            && self.child.is_none()
            && !self.bind_acknowledged
            && self.real_exit.is_none()
    }

    pub fn prepare_sim(&mut self, root: &Path) -> Result<(), String> { self.prepare(root, Kind::SimCodec) }

    /// Prepare and retain the original registration before any resolve attempt.
    /// `PendingRegistration::new_kind` performs only its read-only epoch query;
    /// retries call `resolve` on this stored value and never reconstruct a request.
    pub(crate) fn prepare(&mut self, root: &Path, kind: Kind) -> Result<(), String> {
        if !self.is_empty() {
            return Err("startup_owner_not_empty".into());
        }
        let pending = PendingRegistration::new_kind(root, kind)?;
        self.pending = Some(pending);
        Ok(())
    }

    /// Resolve the exact retained registration. Errors leave it in place for retry.
    pub fn resolve(&mut self) -> Result<(), String> {
        if self.supervisor.is_some() {
            return Ok(());
        }
        if self.child.is_some() || self.real_exit.is_some() {
            return Err("startup_owner_resolve_after_spawn".into());
        }
        let supervisor = self
            .pending
            .as_mut()
            .ok_or("startup_owner_pending_missing")?
            .resolve()?;
        // Install the returned owner before dropping the transferred request.
        self.supervisor = Some(supervisor);
        self.pending = None;
        Ok(())
    }

    /// Configure a command from the held supervisor before spawning it.
    /// The no-broker path leaves command configuration to its caller.
    pub fn configure(&self, command: &mut Command) -> Result<(), String> {
        if let Some(supervisor) = self.supervisor.as_ref() {
            supervisor.configure(command)?;
        } else if self.pending.is_some() {
            return Err("startup_owner_registration_unresolved".into());
        }
        Ok(())
    }

    /// Spawn exactly one child and retain it before attempting broker Bind.
    /// If Bind fails, the same child and supervisor remain owned for wait/retry.
    pub fn spawn(&mut self, command: &mut Command) -> Result<u32, String> {
        if self.child.is_some() || self.real_exit.is_some() {
            return Err("startup_owner_child_already_owned".into());
        }
        if self.pending.is_some() && self.supervisor.is_none() {
            return Err("startup_owner_registration_unresolved".into());
        }
        self.configure(command)?;
        let child = command.spawn().map_err(|error| error.to_string())?;
        let pid = child.id();
        self.child = Some(child);
        self.real_exit = None;
        self.bind_owned_child()?;
        Ok(pid)
    }

    /// Bind the same retained child to the held supervisor. Safe to retry
    /// after a transport error because the original supervisor retains the PID.
    pub fn bind_owned_child(&mut self) -> Result<(), String> {
        if self.bind_acknowledged {
            return Ok(());
        }
        let pid = self.child.as_ref()
            .ok_or("startup_owner_child_missing")?
            .id();
        if let Some(supervisor) = self.supervisor.as_mut() {
            supervisor.bind(pid)?;
            self.bind_acknowledged = true;
        }
        Ok(())
    }

    /// Borrow the exact owned child; callers cannot substitute a replacement.
    pub fn child(&self) -> Result<&Child, String> {
        self.child.as_ref().ok_or_else(|| "startup_owner_child_missing".into())
    }

    /// Mutably borrow the exact owned child for existing IPC operations.
    pub fn child_mut(&mut self) -> Result<&mut Child, String> {
        self.child.as_mut().ok_or_else(|| "startup_owner_child_missing".into())
    }

    /// Report the bound PID when present, rejecting inconsistent cached state.
    pub fn pid_opt(&self) -> Result<Option<u32>, String> {
        match (self.child.as_ref(), self.real_exit) {
            (Some(child), _) => Ok(Some(child.id())),
            (None, None) => Ok(None),
            (None, Some(_)) => Err("startup_owner_exit_without_child".into()),
        }
    }

    /// Return only the status produced by a real wait; never infer it from PID or poll state.
    pub fn real_exit_status(&self) -> Result<Option<ExitStatus>, String> {
        match (self.child.as_ref(), self.real_exit) {
            (Some(_), status) => Ok(status),
            (None, None) => Ok(None),
            (None, Some(_)) => Err("startup_owner_exit_without_child".into()),
        }
    }

    /// Poll the exact owned child. Any actual OS exit status is cached before
    /// returning, so later retirement cannot lose it across broker failures.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
        if let Some(status) = self.real_exit {
            return Ok(Some(status));
        }
        let status = self.child_mut()?.try_wait().map_err(|error| error.to_string())?;
        if let Some(status) = status {
            self.real_exit = Some(status);
        }
        Ok(status)
    }

    /// Wait for the stored child and cache the actual OS status immediately.
    pub fn wait(&mut self) -> Result<ExitStatus, String> {
        if let Some(status) = self.real_exit {
            return Ok(status);
        }
        let status = self.child_mut()?.wait().map_err(|error| error.to_string())?;
        self.real_exit = Some(status);
        Ok(status)
    }

    /// Kill and wait for the stored child. A failed kill is checked for a
    /// concurrently completed child before returning; any real wait is cached.
    pub fn kill_wait(&mut self) -> Result<ExitStatus, String> {
        if let Some(status) = self.real_exit_status()? {
            return Ok(status);
        }
        if let Some(status) = self.child_mut()?.try_wait().map_err(|error| error.to_string())? {
            self.real_exit = Some(status);
            return Ok(status);
        }
        if let Err(kill_error) = self.child_mut()?.kill() {
            if let Some(status) = self.child_mut()?.try_wait().map_err(|error| error.to_string())? {
                self.real_exit = Some(status);
                return Ok(status);
            }
            return Err(kill_error.to_string());
        }
        self.wait()
    }

    /// Retire through the original supervisor protocol. No owned capability is
    /// removed until revoke/ack (or unspawned cancel/ack) has returned success.
    pub fn retire(&mut self) -> Result<(), String> {
        // A failed resolve may have reached the broker even if its reply was
        // lost. Retry the original retained request, then explicitly cancel
        // the returned supervisor; never infer that pending means unallocated.
        if self.pending.is_some() && self.supervisor.is_none() {
            self.resolve()?;
        }

        if self.child.is_some() {
            let pid = self.child.as_ref().ok_or("startup_owner_child_missing")?.id();
            // Reaping the owned process must not depend on broker availability.
            // The original Supervisor retains its PID across a Bind RPC error;
            // after the real wait, it remains the authority for Revoke/Ack.
            let status = self.wait()?;
            if let Some(supervisor) = self.supervisor.as_mut() {
                supervisor.after_sim_wait(pid, status)?;
            }
            // No fallible work follows the authoritative acknowledgement.
            self.child = None;
            self.real_exit = None;
            self.supervisor = None;
            self.bind_acknowledged = false;
            self.pending = None;
            return Ok(());
        }

        if self.real_exit.is_some() {
            return Err("startup_owner_exit_without_child".into());
        }
        if let Some(supervisor) = self.supervisor.as_mut() {
            supervisor.cancel_unspawned()?;
            self.supervisor = None;
        }
        // resolve above established the original supervisor. Ambiguous pending
        // registrations are never discarded by this path.
        self.pending = None;
        self.bind_acknowledged = false;
        Ok(())
    }
}
