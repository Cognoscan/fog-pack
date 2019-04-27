
#[derive(Clone)]
pub struct Permission {
    pub advertise: bool,
    pub machine_local: bool,
    pub direct: bool,
    pub local_net: bool,
    pub global: bool,
    pub anonymous: bool,
}

impl Default for Permission {
    fn default() -> Self {
        Permission::new()
    }
}

impl Permission {
    pub fn new() -> Permission {
        Permission {
            advertise: false,
            machine_local: false,
            direct: false,
            local_net: false,
            global: false,
            anonymous: false,
        }
    }

    /// Whether to advertise a document or not. This is ignored for entries and queries.
    pub fn advertise(mut self, yes: bool) -> Self {
        self.advertise = yes;
        self
    }

    /// Whether this can be shared with other processes on the same machine
    pub fn machine_local(mut self, yes: bool) -> Self {
        self.machine_local = yes;
        self
    }

    /// Whether this can be shared with a node that is directly connected.
    ///
    /// This includes nodes reached via non-mesh Bluetooth, Wi-Fi Direct, direct cable connection, 
    /// etc.
    pub fn direct(mut self, yes: bool) -> Self {
        self.direct = yes;
        self
    }

    /// Whether this can be shared with a node on the local network.
    ///
    /// This includes nodes reached via local Wi-Fi, mesh Wi-Fi, or mesh Bluetooth.
    pub fn local_net(mut self, yes: bool) -> Self {
        self.local_net = yes;
        self
    }

    /// Whether this can be shared with a node anywhere non-local.
    ///
    /// This is for nodes anywhere on the internet.
    pub fn global(mut self, yes: bool) -> Self {
        self.global = yes;
        self
    }

    /// Whether this should be shared anonymously. This generally increases latency and decreases 
    /// bandwidth. 
    ///
    /// This means the underlying connections to other nodes always use anonymizing routing 
    /// methods. Examples include onion routing, garlic routing, and mix networks. Compromises may 
    /// still be possible through careful traffic analysis, especially if non-anonymous documents & 
    /// queries are used.
    pub fn anonymous(mut self, yes: bool) -> Self {
        self.anonymous = yes;
        self
    }
}

