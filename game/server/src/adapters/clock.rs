use std::time::{SystemTime, UNIX_EPOCH};

use agora_domain::Timestamp;
use agora_spi::Clock;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before 1970")
            .as_millis();
        Timestamp(u64::try_from(millis).expect("timestamp fits in u64"))
    }
}
