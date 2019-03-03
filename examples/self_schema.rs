
extern crate condense_db;

use std::collections::BTreeMap;

use condense_db::*;
use condense_db::msgpack;

fn main() {

    let schema = msgpack!({
        "$schema": Hash::new_empty(),
        "name": "Condense-db Certificate List Schema",
        "required": [
            { "name": "id", "type": "Ident" }
        ],
        "entries": [
            {
                "name": "cert",
                "type": "Obj",
                "required": [
                    { "name": "name",  "type": "Str", "max_len": 255 },
                    { "name": "begin", "type": "Time"  },
                    { "name": "end",   "type": "Time"  },
                    { "name": "value", "type": "Int"   }
                ]
            }
        ]
    });

    println!("{}", schema);

}
