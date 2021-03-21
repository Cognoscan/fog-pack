/*!

The main specifications for fog-pack.

fog-pack's specifications cover the raw encoding of data, wrapping that data with a brief header 
and optional compression, and specifying valid data for a schema or a query.

- [Raw Data Format](./raw_data/index.html)
- [Encoding Documents, Entries, and Queries](./encodings/index.html)
- [Schema Document Format](./schema/index.html)
- [Validation Language](./validation/index.html)

*/

pub mod raw_data;
pub mod encodings;
pub mod validation;
pub mod schema;
