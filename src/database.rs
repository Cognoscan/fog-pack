use std::collections::HashMap;
use crossbeam_channel::{TryRecvError, RecvError, Sender, Receiver, unbounded, bounded, Select};

use super::{Permission, Query, Value, Hash, Document, Entry};

/// Types of changes that can be made to the database.
enum ChangeRequest {
    /// Add a document to the databse.
    AddDoc((Document, Permission, u32)),
    /// Remove a document from the database by hash.
    DelDoc(Hash),
    /// Add an entry to the database.
    AddEntry((Entry, u32)),
    /// Remove an entry from the database by hash of document & entry.
    DelEntry((Hash, Hash)),
    /// Remove documents/entries that match the query given.
    DelQuery(Query),
    /// Set time-to-live for a given document.
    SetTtlDoc((Hash, u32)),
    /// Set time-to-live for a given entry, by hash of document & entry.
    SetTtlEntry((Hash, Hash, u32)),
}

/// Result of any changes requested of the database.
#[derive(Debug)]
pub enum ChangeResult {
    /// Change made successfuly
    Ok,
    /// Failed for unknown reasons, likely due to underlying database
    Failed,
    /// Couldn't operate on a document because it doesn't exist. Also returned if adding an entry 
    /// to a non-existent document, or querying a non-existent document.
    NoSuchDoc,
    /// Couldn't operate on an entry because it doesn't exist.
    NoSuchEntry,
    /// Couldn't delete a schema document because it is in use.
    SchemaInUse,
    /// Couldn't delete a certificate because it is in use.
    CertInUse,
    /// Document/Entry failed schema check when being added.
    FailedSchemaCheck,
    /// Couldn't find schema referenced by the document.
    SchemaNotFound,
    /// Query didn't match schema of the document(s) being operated on.
    InvalidQuery,
}

/// Control operations on the database. For housekeeping and starting/stopping the database.
enum DbControl {
    Stop,
}

/// A bundled query request for the system
struct QueryRequest {
    query:  Query,
    permission: Permission,
}

/// Possible responses to a query.
pub enum QueryResponse {
    /// A document matching the query, along with the relative effort spent retrieving it.
    Doc((Document, i32)),
    /// An entry matching the query, along with the relative effort spent retrieving it.
    Entry((Entry, i32)),
    /// Query has been exhausted. Only occurs for queries made to retrive a set list of documents. 
    /// The query channel is closed after this.
    DoneForever,
    /// Query failed to match the schema for all given documents. The query channel is closed after 
    /// this.
    Invalid,
    /// One of the documents retrieved failed schema validation.
    BadDoc(Hash),
    /// One of the documents retrieved used an unknown schema.
    UnknownSchema((Hash, Hash)),
}

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
    pub fn add_doc(&self, doc: Document, perm: &Permission, ttl: u32) -> Result<ChangeWait, ()> {
        self.make_change(ChangeRequest::AddDoc((doc, perm.clone(), ttl)))
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

    pub fn query(&self, query: Query, perm: &Permission, capacity: usize) -> Result<QueryWait, ()> {
        if capacity == 0 { return Err(()); }
        let (result_in, result_out) = bounded(capacity);
        let request = QueryRequest { query: query, permission: perm.clone() };
        self.query_in.send((request, result_in)).map_err(|_e| ())?;
        Ok(QueryWait { chan: result_out })
    }

}

/// A channel that receives the result of a change request.
pub struct ChangeWait {
    chan: Receiver<ChangeResult>
}

impl ChangeWait {
    /// Block until the database change request completes. Errors if the change result has been 
    /// received already, or if the database process died.
    pub fn recv(self) -> Result<ChangeResult, RecvError> {
        self.chan.recv()
    }

    /// Check to see if the change request has completed. Errors if channel is not ready, or if the 
    /// channel disconnected.
    pub fn try_recv(&self) -> Result<ChangeResult, TryRecvError> {
        self.chan.try_recv()
    }
}

pub struct QueryWait {
    chan: Receiver<QueryResponse>
}

impl QueryWait {
    /// Block until a query response occurs. Errors if the database closed the channel, which 
    /// happens if the database stopped or if the database has already sent `DoneForever` or 
    /// `Invalid` responses.
    pub fn recv(&self) -> Result<QueryResponse, RecvError> {
        self.chan.recv()
    }

    /// Check to see if any query results showed up. Errors if channel is not ready, or if the 
    /// channel is closed. Channel is closed if the database stopped, or if the database has 
    /// already sent `DoneForever` or `Invalid` responses.
    pub fn try_recv(&self) -> Result<QueryResponse, TryRecvError> {
        self.chan.try_recv()
    }
}

struct InternalDb {
    doc_db: HashMap<Hash,(Document,Permission,u32)>,
    entry_db: HashMap<Hash, Vec<(Entry,u32)>>,
}

impl InternalDb {
    fn new() -> InternalDb {
        InternalDb {
            doc_db: HashMap::new(),
            entry_db: HashMap::new()
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


