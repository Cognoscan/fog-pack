use std::collections::BTreeMap;
use crossbeam_channel::{Sender, Receiver, unbounded, bounded, Select};

use super::{Hash, Document, Entry};

/// Database for holding documents and associated entries. Tracks schema, handles queries.
pub struct Db {
    handle: std::thread::JoinHandle<()>,
    control_in: Sender<DbControl>,
    change_in: Sender<(ChangeRequest, Sender<ChangeResult>)>,
    query_in: Sender<(QueryRequest, Sender<QueryResponse>)>,
}

impl Db {
    pub fn new() -> Db {
        let (control_in, control_out) = unbounded();
        let (change_in, change_out) = unbounded();
        let (query_in, query_out) = unbounded();
        let handle = std::thread::spawn(move || db_loop(control_out, change_out, query_out));
        Db {
            handle,
            control_in,
            change_in,
            query_in
        }
    }

    /// Submit a change request to the database. Returns a `ChangeWait`, which will eventually 
    /// contain the result of the change operation.
    fn make_change(&self, change: ChangeRequest) -> Result<ChangeWait, ()> {
        let (result_in, result_out) = bounded(1);
        self.change_in.send((change, result_in)).map_err(|_e| ())?;
        Ok(ChangeWait { chan: result_out })
    }

    /// Close out the database and safely exit. Blocks until the thread finishes. Returns an error 
    /// if the thread panicked or the control handle was already closed.
    pub fn close(self) -> Result<(), ()> {
        self.control_in.send(DbControl::Stop).map_err(|_e| ())?;
        self.handle.join().map_err(|_e| ())
    }

    /// Add a document to the database. Returns a `ChangeWait` if request is successfuly made.
    pub fn add_doc(&self, doc: Document, perm: Permission, ttl: u32) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::AddDoc((doc, perm, ttl)))
    }

    /// Remove a document from the database, based on its hash. Returns a `ChangeWait` if request 
    /// is successfully made.
    pub fn del_doc(&self, hash: Hash) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::DelDoc(hash))
    }

    /// Add an entry into the database. Returns a `ChangeWait` if request is successfully made.
    pub fn add_entry(&self, entry: Entry, ttl: u32) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::AddEntry((entry, ttl)))
    }

    /// Remove an entry from the database. Returns a `ChangeWait` if request is successfully made.
    pub fn del_entry(&self, doc_hash: Hash, entry_hash: Hash) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::DelEntry((doc_hash, entry_hash)))
    }

    /// Delete documents and/or entries that match a given query.
    pub fn del_query(&self, query: Query) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::DelQuery(query))
    }

    /// Change the time-to-live (TTL) of a document.
    pub fn set_ttl_doc(&self, doc: Hash, ttl: u32) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::SetTtlDoc((doc, ttl)))
    }

    /// Change the time-to-live (TTL) of an entry.
    pub fn set_ttl_entry(&self, doc: Hash, entry: Hash, ttl: u32) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::SetTtlEntry((doc, entry, ttl)))
    }

    pub fn query(&self, query: Query, perm: Permission, capacity: usize) -> Result<QueryWait, ()> {
        if capacity == 0 { return Err(()); }
        let (result_in, result_out) = bounded(capacity);
        let request = QueryRequest { query: query, permission: perm };
        self.query_in.send((request, result_in)).map_err(|_e| ())?;
        Ok(QueryWait { chan: result_out })
    }

}

pub struct ChangeWait {
    chan: Receiver<ChangeResult>
}

impl ChangeWait {
    /// Block until the database change request completes.
    pub fn recv(self) -> ChangeResult {
        self.chan.recv().unwrap_or(ChangeResult::Failed)
    }

    /// Check to see if the change request has completed
    pub fn try_recv(&self) -> Option<ChangeResult> {
        let res = self.chan.try_recv();
        match res {
            Err(e) => {
                if e.is_empty() {
                    None
                }
                else {
                    Some(ChangeResult::Failed)
                }
            },
            Ok(r) => Some(r),
        }
    }
}

pub struct QueryWait {
    chan: Receiver<QueryResponse>
}

impl QueryWait {
    /// Block until a query response occurs.
    /// Check to see if a new query response is available.
    pub fn recv(&self) -> QueryResponse {
        self.chan.recv().unwrap_or(QueryResponse::DoneForever) // Done forever if channel is closed
    }

    /// The internal database object. Used within the main event loop for storage and various 
    /// operations.
    pub fn try_recv(&self) -> Option<QueryResponse> {
        let res = self.chan.try_recv();
        match res {
            Err(e) => {
                if e.is_empty() {
                    None
                }
                else {
                    Some(QueryResponse::DoneForever) // Done forever if channel is closed
                }
            },
            Ok(r) => Some(r),
        }
    }
}

struct InternalDb {
    doc_db: BTreeMap<Hash,(Document,Permission,u32)>,
    entry_db: BTreeMap<Hash, Vec<(Entry,u32)>>,
}

impl InternalDb {
    fn new() -> InternalDb {
        InternalDb {
            doc_db: BTreeMap::new(),
            entry_db: BTreeMap::new()
        }
    }

    fn make_change(&mut self, change: ChangeRequest) -> ChangeResult {
        match change {
            ChangeRequest::AddDoc((doc, perm, ttl)) => {
                let hash = doc.hash();
                if !self.doc_db.contains_key(&hash) {
                    self.doc_db.insert(hash, (doc, perm, ttl));
                }
                ChangeResult::Ok
            },
            ChangeRequest::DelDoc(hash)     => {
                if !self.doc_db.contains_key(&hash) {
                    ChangeResult::NoSuchDoc
                }
                else {
                    self.doc_db.remove(&hash);
                    ChangeResult::Ok
                }
            },
            ChangeRequest::AddEntry(_)      => ChangeResult::Failed,
            ChangeRequest::DelEntry(_)      => ChangeResult::Failed,
            ChangeRequest::DelQuery(_)      => ChangeResult::Failed,
            ChangeRequest::SetTtlDoc(_)     => ChangeResult::Failed,
            ChangeRequest::SetTtlEntry(_)   => ChangeResult::Failed,
        }
    }
}

/// The primary event loop for the database. The actual database handles change requests, queries, 
/// and management in this loop, which should be running in its own thread. It will block until one 
/// of the following conditions are met:
///
/// - A query response channel goes from full to open / disconnected.
/// - A change request has been made
/// - A database management command has been issued
/// 
fn db_loop(
    control: Receiver<DbControl>,
    change: Receiver<(ChangeRequest, Sender<ChangeResult>)>,
    query_inbox: Receiver<(QueryRequest, Sender<QueryResponse>)>)
{
    let mut db = InternalDb::new();
    let mut done = false;

    // Set up queries to respond to
    let mut open_queries: Vec<(QueryRequest, Sender<QueryResponse>)> = Vec::new();
    
    // Set up the channel selector
    let mut select = Select::new();
    let index_ctrl = select.recv(&control);
    let index_change = select.recv(&change);
    let index_query = select.recv(&query_inbox);

    // Main loop
    loop {
        let mut active = false;
        let oper = select.try_select();
        if let Ok(oper) = select.try_select() {
            active = true;
            match oper.index() {
                i if i == index_ctrl => {
                    let cmd = oper.recv(&control);
                    match cmd {
                        Err(_) => done = true, // Halt if command channel was closed
                        Ok(cmd) => {
                            match cmd {
                                DbControl::Stop => done = true,
                            }
                        }
                    };
                },
                i if i == index_change => {
                    if let Ok((cmd, resp)) = oper.recv(&change) {
                        // Send the response. If nothing is at the other end, we don't care.
                        resp.send(db.make_change(cmd)).unwrap_or(());
                    }
                },
                i if i == index_query => {
                    if let Ok((query, resp)) = oper.recv(&query_inbox) {
                        open_queries.push((query, resp));
                    }
                }
                _ => unreachable!(),
            }
        }

        open_queries.retain(|(query_request, response)| {
            response.send(QueryResponse::DoneForever).unwrap_or(());
            drop(response);
            false
        });

        if done {
            break;
        }

        // Yield to the OS if the database is idling
        if !active {
            std::thread::yield_now();
        }
    };
}

#[derive(Clone)]
pub struct Permission {
    advertise: bool,
    machine_local: bool,
    direct: bool,
    local_net: bool,
    global: bool,
    anonymous: bool,
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
    pub fn advertise(&mut self, yes: bool) {
        self.advertise = yes;
    }

    /// Whether this can be shared with other processes on the same machine
    pub fn machine_local(&mut self, yes: bool) {
        self.machine_local = yes;
    }

    /// Whether this can be shared with a node that is directly connected.
    ///
    /// This includes nodes reached via non-mesh Bluetooth, Wi-Fi Direct, direct cable connection, 
    /// etc.
    pub fn direct(&mut self, yes: bool) {
        self.direct = yes;
    }

    /// Whether this can be shared with a node on the local network.
    ///
    /// This includes nodes reached via local Wi-Fi, mesh Wi-Fi, or mesh Bluetooth.
    pub fn local_net(&mut self, yes: bool) {
        self.local_net = yes;
    }

    /// Whether this can be shared with a node anywhere non-local.
    ///
    /// This is for nodes anywhere on the internet.
    pub fn global(&mut self, yes: bool) {
        self.global = yes;
    }

    /// Whether this should be shared anonymously. This generally increases latency and decreases 
    /// bandwidth. 
    ///
    /// This means the underlying connections to other nodes always use anonymizing routing 
    /// methods. Examples include onion routing, garlic routing, and mix networks. Compromises may 
    /// still be possible through careful traffic analysis, especially if non-anonymous documents & 
    /// queries are used.
    pub fn anonymous(&mut self, yes: bool) {
        self.anonymous = yes;
    }
}

#[derive(Clone)]
pub struct Query {
    pub query: Vec<u8>,
}

pub enum ChangeRequest {
    AddDoc((Document, Permission, u32)),
    DelDoc(Hash),
    AddEntry((Entry, u32)),
    DelEntry((Hash, Hash)),
    DelQuery(Query),
    SetTtlDoc((Hash, u32)),
    SetTtlEntry((Hash, Hash, u32)),
}

#[derive(Debug)]
pub enum ChangeResult {
    Ok,
    Failed,
    NoSuchDoc,
    NoSuchEntry,
    SchemaInUse,
    CertInUse,
    FailedSchemaCheck,
    SchemaNotFound,
    InvalidQuery,
}

enum DbControl {
    Stop,
}

struct QueryRequest {
    query:  Query,
    permission: Permission,
}

pub enum QueryResponse {
    Doc((Document, i32)),
    Entry((Entry, i32)),
    DoneForever,
    Invalid,
    BadDoc(Hash),
}





