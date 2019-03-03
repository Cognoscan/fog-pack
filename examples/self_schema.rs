
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

    let mut core_schema = msgpack!({
        "$schema": Hash::new_empty(),
        "name": "Condense-db Core Schema",
        "version": 1,
        "required": [
            { "name": "name", "type": "Str" }
        ],
        "optional": [
            { "name": "comment",    "type": "Str" },
            { "name": "version",    "type": "Int" },
            { "name": "required",   "type": "Array", "items": "Type", "unique_fields": ["name"], "comment": "All fields that must be in the schema"  },
            { "name": "optional",   "type": "Array", "items": "Type", "unique_fields": ["name"], "comment": "All optional fields that may be in the schema"  },
            { "name": "entries",    "type": "Array", "items": "Type", "unique_fields": ["name"], "comment": "All fields that entries could be"  },
            { "name": "types",      "type": "Array", "items": "Type", "unique_fields": ["name"], "comment": "All complex types used in the schema"  },
            { "name": "unknown_ok", "type": "Bool" , "comment": "Set true if additional unknown fields are allowed"  }
        ],
        "types": [ ]
    });

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "Type",
            "type": "Multi",
            "any_of": [
                "NullType", "BoolType",  "IntType",   "StrType",
                "F32Type",  "F64Type",   "BinType",   "ArrayType",
                "ObjType",  "HashType",  "IdentType", "LockType",
                "TimeType", "MultiType", "OtherType"
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "NullType",
            "type": "Obj",
            "comment": "Null type, can only be null, but can be queried",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Null" }
            ],
            "optional": [
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "BoolType",
            "type": "Obj",
            "comment": "Boolean type, can be true or false",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Bool" }
            ],
            "optional": [
                { "name": "default", "type": "Bool" },
                { "name": "comment", "type": "Str"  },
                { "name": "const",   "type": "Bool" },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "IntType",
            "type": "Obj",
            "comment": "Integer type, can range from -2^63 to 2^64-1",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Int" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "Int", "unique": true },
                { "name": "nin",     "type": "Array", "items": "Int", "unique": true },
                { "name": "const",   "type": "Int"  },
                { "name": "min",     "type": "Int"  },
                { "name": "max",     "type": "Int"  },
                { "name": "ex_min",  "type": "Bool" },
                { "name": "ex_max",  "type": "Bool" },
                { "name": "default", "type": "Int"  },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" },
                { "name": "bit",     "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "StrType",
            "type": "Obj",
            "comment": "String type, can hold arbitrary byte string",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Str" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "Str", "unique": true },
                { "name": "nin",     "type": "Array", "items": "Str", "unique": true },
                { "name": "const",   "type": "Str"  },
                { "name": "min_len", "type": "Int", "min": 0 },
                { "name": "max_len", "type": "Int", "min": 0 },
                { "name": "matches", "type": "Str"  },
                { "name": "default", "type": "Str"  },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" },
                { "name": "regex",   "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "F32Type",
            "type": "Obj",
            "comment": "32-bit floating type",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "F32" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "F32", "unique": true },
                { "name": "nin",     "type": "Array", "items": "F32", "unique": true },
                { "name": "const",   "type": "F32"  },
                { "name": "min",     "type": "F32"  },
                { "name": "max",     "type": "F32"  },
                { "name": "ex_min",  "type": "Bool" },
                { "name": "ex_max",  "type": "Bool" },
                { "name": "default", "type": "F32"  },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "F64Type",
            "type": "Obj",
            "comment": "64-bit floating type",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "F64" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "F64", "unique": true },
                { "name": "nin",     "type": "Array", "items": "F64", "unique": true },
                { "name": "const",   "type": "F64"  },
                { "name": "min",     "type": "F64"  },
                { "name": "max",     "type": "F64"  },
                { "name": "ex_min",  "type": "Bool" },
                { "name": "ex_max",  "type": "Bool" },
                { "name": "default", "type": "F64"  },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "BinType",
            "type": "Obj",
            "comment": "Binary type, can hold arbitrary byte sequence",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Bin" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "Bin", "unique": true },
                { "name": "nin",     "type": "Array", "items": "Bin", "unique": true },
                { "name": "const",   "type": "Bin"  },
                { "name": "min_len", "type": "Int", "min": 0 },
                { "name": "max_len", "type": "Int", "min": 0 },
                { "name": "default", "type": "Bin"  },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "bit",     "type": "Bool" },
                { "name": "ord",     "type": "Bool" }
            ]
        }
    ));

    let mut array_type = msgpack!(
        {
            "name": "ArrayType",
            "type": "Obj",
            "comment": "Array type, can hold arrays of values",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Array" }
            ],
            "optional": [ ]
        }
    );
    array_type["optional"].as_array_mut().unwrap().append(&mut vec![
        msgpack!( { "name": "in",            "type": "Array", "items": "Array", "unique": true } ),
        msgpack!( { "name": "nin",           "type": "Array", "items": "Array", "unique": true } ),
        msgpack!( { "name": "const",         "type": "Array" } ),
        msgpack!( { "name": "items",         "type": "Multi", "any_of": ["StrName", "StrArray"] } ),
        msgpack!( { "name": "extra_items",   "type": "StrName"   } ),
        msgpack!( { "name": "contains",      "type": "Multi", "any_of": ["StrName", "StrArray"] } ),
        msgpack!( { "name": "unique_fields", "type": "Array", "items": "Str", "unique": true } ),
        msgpack!( { "name": "min_len",       "type": "Int", "min": 0 } ),
        msgpack!( { "name": "max_len",       "type": "Int", "min": 0 } ),
        msgpack!( { "name": "unique",        "type": "Bool"  } ),
        msgpack!( { "name": "default",       "type": "Array" } ),
        msgpack!( { "name": "comment",       "type": "Str"   } ),
        msgpack!( { "name": "query",         "type": "Bool"  } ),
        msgpack!( { "name": "array",         "type": "Bool"  } )
    ]);
    core_schema["types"].as_array_mut().unwrap().push(array_type);

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "ObjType",
            "type": "Obj",
            "comment": "Object type: any key-value map",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Obj" }
            ],
            "optional": [
                { "name": "in",         "type": "Array", "items": "Obj", "unique": true },
                { "name": "nin",        "type": "Array", "items": "Obj", "unique": true },
                { "name": "const",      "type": "Obj"     },
                { "name": "required",   "type": "Array", "items": "Type", "unique_fields": ["name"] },
                { "name": "optional",   "type": "Array", "items": "Type", "unique_fields": ["name"] },
                { "name": "min_fields", "type": "Int", "min": 0 },
                { "name": "max_fields", "type": "Int", "min": 0 },
                { "name": "field_type", "type": "StrName" },
                { "name": "default",    "type": "Obj"     },
                { "name": "comment",    "type": "Str"     },
                { "name": "query",      "type": "Bool"    },
                { "name": "unknown_ok", "type": "Bool"    }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "HashType",
            "type": "Obj",
            "comment": "Hash type: any cryptographihc hash",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Hash" }
            ],
            "optional": [
                { "name": "in",         "type": "Array", "items": "Hash", "unique": true },
                { "name": "nin",        "type": "Array", "items": "Hash", "unique": true },
                { "name": "const",      "type": "Hash" },
                { "name": "default",    "type": "Hash" },
                { "name": "comment",    "type": "Str" },
                { "name": "query",      "type": "Bool" },
                { "name": "link",       "type": "Multi", "any_of": [ "Hash", "HashArray" ] }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "IdentType",
            "type": "Obj",
            "comment": "Identity type: any cryptographic public key",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Ident" }
            ],
            "optional": [
                { "name": "comment",    "type": "Str" },
                { "name": "query",      "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "LockType",
            "type": "Obj",
            "comment": "Lockbox type: Encrypted data, either secret/private keys or an encrypted value",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Lock" }
            ],
            "optional": [
                { "name": "comment",    "type": "Str" },
                { "name": "query",      "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "TimeType",
            "type": "Obj",
            "comment": "Timestamp type",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Time" }
            ],
            "optional": [
                { "name": "in",      "type": "Array", "items": "Time", "unique": true },
                { "name": "nin",     "type": "Array", "items": "Time", "unique": true },
                { "name": "const",   "type": "Time" },
                { "name": "min",     "type": "Time" },
                { "name": "max",     "type": "Time" },
                { "name": "ex_min",  "type": "Bool" },
                { "name": "ex_max",  "type": "Bool" },
                { "name": "default", "type": "Time" },
                { "name": "comment", "type": "Str"  },
                { "name": "query",   "type": "Bool" },
                { "name": "ord",     "type": "Bool" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "MultiType",
            "type": "Obj",
            "comment": "Type that can be one of multiple types",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str", "const": "Multi" },
                { "name": "any_of",   "type": "Array", "items": "StrName", "unique": true }
            ],
            "optional": [
                { "name": "comment", "type": "Str" }
            ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "OtherType",
            "type": "Obj",
            "comment": "Type Definition used to reference complex types",
            "required": [
                { "name": "name", "type": "StrName" },
                { "name": "type", "type": "Str" }
            ],
            "optional": [ { "name": "comment", "type": "Str" } ]
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "StrName",
            "type": "Str",
            "comment": "Used to validate type names",
            "matches": "[^$].*"
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "StrArray",
            "type": "Array",
            "items": "StrName",
            "comment": "Used for the items field in Array types"
        }
    ));

    core_schema["types"].as_array_mut().unwrap().push(msgpack!(
        {
            "name": "HashArray",
            "type": "Array",
            "items": "Hash",
            "comment": "Used for the link field in Hash types"
        }
    ));

    println!("{}", core_schema);

}
