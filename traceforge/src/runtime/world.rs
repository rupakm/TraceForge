use super::host::Host;
use crate::net::dns::{Dns, ToIpAddr, ToIpAddrs};
use crate::net::ip::IpVersionAddrIter;
use crate::thread::ThreadId;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

pub(crate) struct World {
    /// Tracks all individual hosts
    pub(crate) hosts: HashMap<IpAddr, Host>,

    /// Tracks how each host is connected to each other.
    // pub(crate) topology: Topology,
    dns: Dns,
    tidmap: HashMap<SocketAddr, ThreadId>,

    uuid: UidGenerator,

    current: Option<IpAddr>,
}

impl World {
    pub fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            dns: Dns::new(IpVersionAddrIter::default()),
            tidmap: HashMap::new(),
            uuid: UidGenerator::new(),
            current: None,
        }
    }

    pub fn dns(&mut self) -> &mut Dns {
        &mut self.dns
    }

    pub(crate) fn lookup_tid(&self, s: SocketAddr) -> Option<&ThreadId> {
        self.tidmap.get(&s)
    }

    pub(crate) fn current_host_mut(&mut self) -> &mut Host {
        let addr = self.current.expect("current host missing");
        self.hosts.get_mut(&addr).expect("host missing")
    }

    pub(crate) fn current_host(&self) -> &Host {
        let addr = self.current.expect("current host missing");
        self.hosts.get(&addr).expect("host missing")
    }

    pub(crate) fn lookup(&mut self, host: impl ToIpAddr) -> IpAddr {
        self.dns.lookup(host)
    }

    pub(crate) fn reverse_lookup(&self, addr: IpAddr) -> Option<&str> {
        self.dns.reverse(addr)
    }

    pub(crate) fn lookup_many(&mut self, hosts: impl ToIpAddrs) -> Vec<IpAddr> {
        self.dns.lookup_many(hosts)
    }
}

pub type Uid = u64;

#[derive(Eq, PartialEq, Default, Debug)]
pub(crate) struct UidGenerator {
    current: u64,
}

impl UidGenerator {
    pub fn new() -> Self {
        Self { current: 0 }
    }

    pub fn next(&mut self) -> Uid {
        self.current += 1;
        self.current
    }
}
