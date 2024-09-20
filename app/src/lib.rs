#![no_std]

use sails_rs::prelude::*;
mod session_generaion;
pub struct SessionsProgram(());

#[program]
impl SessionsProgram {
    pub async fn new(config: Config) -> Self {
        SessionService::init(config);
        Self(())
    }

    pub fn session(&self) -> SessionService {
        SessionService::new()
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
