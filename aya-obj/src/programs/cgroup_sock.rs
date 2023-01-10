//! Cgroup socket programs.
use alloc::{borrow::ToOwned, string::String};

use crate::{
    generated::bpf_attach_type,
    thiserror::{self, Error},
};

/// Defines where to attach a `CgroupSock` program.
#[derive(Copy, Clone, Debug)]
pub enum CgroupSockAttachType {
    /// Called after the IPv4 bind events.
    PostBind4,
    /// Called after the IPv6 bind events.
    PostBind6,
    /// Attach to IPv4 connect events.
    SockCreate,
    /// Attach to IPv6 connect events.
    SockRelease,
}

impl Default for CgroupSockAttachType {
    // The kernel checks for a 0 attach_type and sets it to sock_create
    // We may as well do that here also
    fn default() -> Self {
        CgroupSockAttachType::SockCreate
    }
}

impl From<CgroupSockAttachType> for bpf_attach_type {
    fn from(s: CgroupSockAttachType) -> bpf_attach_type {
        match s {
            CgroupSockAttachType::PostBind4 => bpf_attach_type::BPF_CGROUP_INET4_POST_BIND,
            CgroupSockAttachType::PostBind6 => bpf_attach_type::BPF_CGROUP_INET6_POST_BIND,
            CgroupSockAttachType::SockCreate => bpf_attach_type::BPF_CGROUP_INET_SOCK_CREATE,
            CgroupSockAttachType::SockRelease => bpf_attach_type::BPF_CGROUP_INET_SOCK_RELEASE,
        }
    }
}

#[derive(Debug, Error)]
#[error("{0} is not a valid attach type for a CGROUP_SOCK program")]
pub(crate) struct InvalidAttachType(String);

impl CgroupSockAttachType {
    pub(crate) fn try_from(value: &str) -> Result<CgroupSockAttachType, InvalidAttachType> {
        match value {
            "post_bind4" => Ok(CgroupSockAttachType::PostBind4),
            "post_bind6" => Ok(CgroupSockAttachType::PostBind6),
            "sock_create" => Ok(CgroupSockAttachType::SockCreate),
            "sock_release" => Ok(CgroupSockAttachType::SockRelease),
            _ => Err(InvalidAttachType(value.to_owned())),
        }
    }
}
