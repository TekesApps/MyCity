//! Explicit finite disk budget. Normal work cannot consume the emergency reserve.
use super::*;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityPolicy {
    pub total_bytes: u64,
    pub emergency_bytes: u64,
}
impl Default for CapacityPolicy {
    fn default() -> Self {
        Self {
            total_bytes: checkpoint::ARCHIVE_LIMIT,
            emergency_bytes: 8 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct CapacityStatus {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub normal_limit: u64,
    pub normal_blocked: bool,
    pub hard_blocked: bool,
    pub reason: Option<&'static str>,
}
impl CapacityPolicy {
    pub(super) fn validate(self) -> Result<(), Error> {
        if self.total_bytes > checkpoint::ARCHIVE_LIMIT
            || self.emergency_bytes < 4096
            || self.emergency_bytes >= self.total_bytes
        {
            return Err(Error::Invalid("capacity_policy"));
        }
        Ok(())
    }
}
pub(super) fn bytes(dir: &Path) -> Result<u64, Error> {
    let mut n = 0u64;
    for e in fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_file() {
            n = n
                .checked_add(e.metadata()?.len())
                .ok_or(Error::Invalid("capacity_size_overflow"))?;
        }
    }
    Ok(n)
}
pub(super) fn admit(
    dir: &Path,
    policy: CapacityPolicy,
    additional: u64,
    control: bool,
) -> Result<(), Error> {
    policy.validate()?;
    let used = bytes(dir)?;
    let limit = if control {
        policy.total_bytes
    } else {
        policy.total_bytes - policy.emergency_bytes
    };
    if used
        .checked_add(additional)
        .ok_or(Error::Invalid("capacity_size_overflow"))?
        > limit
    {
        return Err(Error::Capacity {
            used_bytes: used,
            limit_bytes: limit,
            required_bytes: additional,
            emergency: control,
        });
    }
    Ok(())
}
impl Store {
    /// A laboratory quota can only be reduced, never enlarged to evade exhaustion.
    /// The root publication persists it across reopening without a business event.
    pub fn reduce_capacity_policy(&mut self, policy: CapacityPolicy) -> Result<(), Error> {
        self.engine()?;
        policy.validate()?;
        if policy.total_bytes > self.io.root.capacity.total_bytes
            || policy.emergency_bytes > self.io.root.capacity.emergency_bytes
        {
            return Err(Error::Invalid("capacity_cannot_enlarge"));
        }
        let result = (|| {
            let mut root = self.io.root.clone();
            root.capacity = policy;
            root.generation = root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?;
            let slot = (root.generation % 2) as u8;
            let encoded = json(&root)?;
            admit(
                &self.io.dir,
                policy,
                encoded.len() as u64 + META_LIMIT,
                true,
            )?;
            atomic_file(&self.io.dir, &format!("root-{slot}.json"), &encoded, || {
                Ok(())
            })?;
            atomic_file(
                &self.io.dir,
                "CURRENT.json",
                &json(&Current {
                    schema: "mycity-m0-current-v1".into(),
                    slot,
                    root_sha256: sha256(&encoded),
                })?,
                || Ok(()),
            )?;
            self.io.root = root;
            Ok(())
        })();
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e, Error::Capacity { .. }))
        {
            self.io.poisoned = true;
        }
        result
    }
    pub fn capacity_status(&self) -> Result<CapacityStatus, Error> {
        self.engine()?;
        let used = bytes(&self.io.dir)?;
        let p = self.io.root.capacity;
        // Reserve a maximum admitted frame and publication metadata. Even an empty file
        // needs durable space for its receipt before the model may advance.
        let next = (FRAME_LIMIT as u64) + META_LIMIT * 2;
        let normal_limit = p.total_bytes - p.emergency_bytes;
        let normal_blocked = used.saturating_add(next) > normal_limit;
        let hard_blocked = used.saturating_add(next) > p.total_bytes;
        Ok(CapacityStatus {
            used_bytes: used,
            total_bytes: p.total_bytes,
            normal_limit,
            normal_blocked,
            hard_blocked,
            reason: if hard_blocked {
                Some("necessary_fact_storage_exhausted")
            } else if normal_blocked {
                Some("normal_storage_reserved_for_existing_responsibilities")
            } else {
                None
            },
        })
    }
}
