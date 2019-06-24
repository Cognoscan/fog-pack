- Break out the invalid query response by having a single response with 
  valid/invalid, and attached queue if valid
- Hide the root document behind API calls. Instead, do something like 
  "AddNamedDoc" for it to stick around, then do everything else within 
	transactions. Assume that the user does not want to do bundled transactions 
	that include name document changes
	- AddNamedDoc
	- DelNamedDoc
	- ReplaceNamedDoc
	- Transaction
		- AddEntry
		- DelEntry
		- DelQuery (on entries)
		- AddDoc
		- DelDoc
		- SetTtlDoc
		- SetTtlEntry
- Rename Document's `extract_schema` to `extract_schema_hash`
- Change Document's `from_raw` to do the signature verification
- Change Entry's `from_raw` to do the signature verification
- Change Document/Entry to get Identity from Signature when calling `sign`
- Change interface to Document/Entry so they give hashes only once they've been 
  submitted to the database...? Maybe not
- Make sure state management for open queries is all within OpenQuery
