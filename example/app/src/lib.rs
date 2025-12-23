#![no_std]

use sails_rs::prelude::*;
use session_service::*;
pub struct SessionsProgram {
    session_storage: RefCell<SessionStorage>,
}

#[sails_rs::program]
impl SessionsProgram {
    pub async fn new(config: Config) -> Self {
        Self {
            session_storage: RefCell::new(SessionStorage::new(config)),
        }
    }

    pub fn session(&self) -> SessionService<'_> {
        SessionService::new(&self.session_storage)
    }
}

#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
#[codec(crate = sails_rs::scale_codec)]
#[scale_info(crate = sails_rs::scale_info)]
pub enum ActionsForSession {
    StartGame,
    Move,
    Skip,
}

generate_session_system!(ActionsForSession);
