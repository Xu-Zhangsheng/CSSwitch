//! 生命周期串行器（spec §8.1，与 native-entry §5.3 共用最小核心）。
//! 所有会改变 runtime context 的操作先声明 typed mutation domain，再复用同一把
//! process-local mutex 串行化；探活刻意在锁外，用 generation token 防
//! 「被清除/取代后又拿旧 key 复活代理」。
//!
//! 三把锁分层，严格避免自死锁：
//!   1. 本串行器锁（`Mutex<()>`）= 最外层，命令级操作整段持有，**绝不重入**；
//!   2. `AppState` 锁 = 内层，读写运行态时短暂持有，探活期间释放；
//!   3. `config::update` 锁 = 最内层，仅盖 load-modify-save。
//!
//! GatewayController **绝不**取本串行器锁（其调用方命令才取），故不自锁。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeMutationDomain {
    Intent,
    Destructive,
    HostBridge,
    Terminal,
}

/// Process-local proof that one typed runtime mutation currently owns the
/// shared lifecycle serializer. Dropping the value releases the lease.
pub(crate) struct RuntimeMutationLease<'a> {
    domain: RuntimeMutationDomain,
    _guard: MutexGuard<'a, ()>,
}

impl RuntimeMutationLease<'_> {
    pub(crate) fn domain(&self) -> RuntimeMutationDomain {
        self.domain
    }
}

pub struct Lifecycle {
    lock: Mutex<()>,
    generation: AtomicU64,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl Lifecycle {
    pub fn new() -> Self {
        Lifecycle {
            lock: Mutex::new(()),
            generation: AtomicU64::new(1),
        }
    }

    pub(crate) fn acquire_mutation(
        &self,
        domain: RuntimeMutationDomain,
    ) -> RuntimeMutationLease<'_> {
        RuntimeMutationLease {
            domain,
            _guard: self.lock.lock().unwrap_or_else(|error| error.into_inner()),
        }
    }

    #[cfg(test)]
    pub(crate) fn try_acquire_mutation(
        &self,
        domain: RuntimeMutationDomain,
    ) -> Option<RuntimeMutationLease<'_>> {
        let guard = match self.lock.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        };
        Some(RuntimeMutationLease {
            domain,
            _guard: guard,
        })
    }

    /// 在 typed app 级 mutation lease 下跑复合操作。poison 也照常恢复继续。
    pub(crate) fn with_mutation<T>(
        &self,
        domain: RuntimeMutationDomain,
        f: impl FnOnce(&RuntimeMutationLease<'_>) -> T,
    ) -> T {
        let lease = self.acquire_mutation(domain);
        f(&lease)
    }

    /// 只为需要与 mutation 串行取得一致 read model 的非 mutation 操作保留。
    pub(crate) fn with_observed_context<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        f()
    }

    #[cfg(test)]
    pub(crate) fn with_serialized<T>(&self, f: impl FnOnce() -> T) -> T {
        self.with_observed_context(f)
    }

    /// 清 key / 停 / 切换时调用：使正在锁外探活的旧启动作废。返回新 generation。
    pub fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_mutation_domains_share_one_exclusive_serializer() {
        let lifecycle = Lifecycle::new();
        let domains = [
            RuntimeMutationDomain::Intent,
            RuntimeMutationDomain::Destructive,
            RuntimeMutationDomain::HostBridge,
            RuntimeMutationDomain::Terminal,
        ];
        for (index, domain) in domains.iter().copied().enumerate() {
            let lease = lifecycle.acquire_mutation(domain);
            assert_eq!(lease.domain(), domain);
            let contender = domains[(index + 1) % domains.len()];
            assert!(
                lifecycle.try_acquire_mutation(contender).is_none(),
                "every typed domain must contend on the same held serializer"
            );
            drop(lease);
            let acquired = lifecycle
                .try_acquire_mutation(contender)
                .expect("dropping the prior domain must release the serializer");
            assert_eq!(acquired.domain(), contender);
            drop(acquired);
        }
    }

    #[test]
    fn generation_bumps_monotonically() {
        let lc = Lifecycle::new();
        let g0 = lc.current_generation();
        let g1 = lc.bump_generation();
        assert!(g1 > g0);
        assert_eq!(lc.current_generation(), g1);
        let g2 = lc.bump_generation();
        assert!(g2 > g1);
    }
}
