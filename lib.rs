//! nonce — a single-wallet EVM nonce manager.
//!
//! Design rule: nonce — тільки з pending-стану ланцюга,
//! жодних паралельних лічильників. This component is the single issuance point
//! of nonces for every component acting for one wallet:
//! - ініціалізація — з `eth_getTransactionCount(pending)`;
//! - `peek()` — nonce для транзакції, що будується (НЕ просуває лічильник);
//! - `mark_accepted()` — інкремент ТІЛЬКИ після того, як вузол прийняв
//!   транзакцію (eth_sendRawTransaction повернув hash без помилки);
//! - `resync(pending)` — скидання до свіжого pending при будь-якій
//!   розбіжності/помилці (nonce too low / already known / replacement).

use alloy::primitives::Address;
use alloy::providers::Provider;
use std::borrow::Cow;

/// Стан менеджера одного гаманця. Не клонувати між лініями: власник —
/// один (orchestrator), лінії отримують nonce тільки через нього.
#[derive(Debug)]
pub struct NonceManager {
    address: Address,
    next: u64,
    accepted_total: u64,
}

impl NonceManager {
    /// Створити з відомого pending-значення (без мережі).
    pub fn from_pending(address: Address, pending: u64) -> Self {
        NonceManager {
            address,
            next: pending,
            accepted_total: 0,
        }
    }

    /// Ініціалізація з живого ланцюга: pending count гаманця.
    pub async fn init<P: Provider>(
        provider: &P,
        address: Address,
    ) -> Result<Self, String> {
        let pending = pending_count(provider, address).await?;
        Ok(Self::from_pending(address, pending))
    }

    pub fn address(&self) -> Address {
        self.address
    }

    /// Nonce для наступної транзакції. Виклик не змінює стан: та сама
    /// транзакція може бути перебудована/перепідписана з тим самим nonce.
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Вузол прийняв транзакцію з `peek()` — просунути лічильник рівно
    /// на одиницю. Викликати ТІЛЬКИ після успішного send (hash отримано).
    pub fn mark_accepted(&mut self) {
        self.next += 1;
        self.accepted_total += 1;
    }

    /// Скидання до свіжого pending-значення (помилка, гонка, рестарт
    /// лінії). Повертає попереднє значення для журналу розбіжностей.
    pub fn resync(&mut self, fresh_pending: u64) -> u64 {
        let old = self.next;
        self.next = fresh_pending;
        old
    }

    /// Пересинхронізація з ланцюга одним викликом.
    pub async fn resync_from_chain<P: Provider>(
        &mut self,
        provider: &P,
    ) -> Result<u64, String> {
        let fresh = pending_count(provider, self.address).await?;
        Ok(self.resync(fresh))
    }

    pub fn accepted_total(&self) -> u64 {
        self.accepted_total
    }
}

/// eth_getTransactionCount(address, "pending") через raw_request —
/// семантика pending явна, без прихованих дефолтів провайдера.
pub async fn pending_count<P: Provider>(
    provider: &P,
    address: Address,
) -> Result<u64, String> {
    let v: serde_json::Value = provider
        .raw_request(
            Cow::Borrowed("eth_getTransactionCount"),
            (format!("{:#x}", address), "pending".to_string()),
        )
        .await
        .map_err(|e| format!("getTransactionCount: {}", e))?;
    let hex = v.as_str().ok_or("getTransactionCount: not a hex string")?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("getTransactionCount parse: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr() -> Address {
        "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap()
    }

    #[test]
    fn peek_does_not_advance() {
        let m = NonceManager::from_pending(addr(), 7);
        assert_eq!(m.peek(), 7);
        assert_eq!(m.peek(), 7);
    }

    #[test]
    fn accepted_advances_by_one() {
        let mut m = NonceManager::from_pending(addr(), 7);
        m.mark_accepted();
        assert_eq!(m.peek(), 8);
        m.mark_accepted();
        assert_eq!(m.peek(), 9);
        assert_eq!(m.accepted_total(), 2);
    }

    #[test]
    fn resync_resets_to_chain_pending() {
        let mut m = NonceManager::from_pending(addr(), 7);
        m.mark_accepted();
        m.mark_accepted();
        let old = m.resync(5);
        assert_eq!(old, 9);
        assert_eq!(m.peek(), 5);
    }
}
