use std::sync::LazyLock;

use const_format::formatcp;
use reqwest::Client;

use crate::built_info;

pub static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

pub const USER_AGENT: &str = formatcp!(
    "{}/{} ({})",
    built_info::PKG_NAME,
    built_info::PKG_VERSION,
    built_info::PKG_HOMEPAGE
);
