use super::*;

use std::rc::Rc;
pub struct Channel {
    pub user_id: u32,
    pub sender: Arc<WsSender>,
}

impl Channel {
    pub fn init(user_id: u32, sender: Arc<WsSender>) -> Self {
        Channel {
            user_id: user_id,
            sender: sender,
        }
    }
}
