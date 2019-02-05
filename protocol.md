# Protocols #

We don't care what the protocol is, exactly, just what it accomplishes:

1. Must establish and maintain a connection with a remote machine
2. The connection must be bi-directional, packetized, and guarantee packets are 
eventually delivered so long as the connection is maintained.
3. It must have some means of detecting a dropped connection, and detecting when 
the remote machine thinks the connection was dropped temporarily.
4. The connection must be encrypted using the best available encryption methods. 
This generally means if it can theoretically provide a guarantee of forward 
secrecy, identity hiding, deniable authentication, etc., it should.

## Capabilities ##

- Locations
	- Self: Can only talk to other implementations within the local host.
	- Direct connection: Can talk directly with a single attached device
	- Immediate area: Can talk to any devices within personal radio range. 
	- Immediate area with range: Can determine range to other devices and 
	  limit to devices within a set distance or outside a set distance.
	- Immediate network: Can talk only on a local area network
	- Area network: Can talk on a larger network that isn't the internet
	- Global network: Can talk to any device on the internet
- Identification
	- Public: Can locate a specific node, but others will know about the search
	- Covert: Can locate a specific node without letting others know about the 
	  search
- Data concealment
	- Source and destination of queries and responses is concealed

## Packet types used ##

- New query
- Replace query
- Delete query
- ACK query
- NACK query (with optional reason)
- Request query list
- Provide query list
- Respond to query

## Optional Capabilities ##

A protocol can optionally meet more than the requirements above, or can specify 
a specific limitation or capability.

### Location Aware ###

Any protocol must declare the possible communication distances, either logical 
or physical. 


If a protocol *can* talk over the local network, but *cannot* limit itself to 
just the local network, then it must choose "Global network" only. Pick only the 
largest network/area that the protocol can talk over, unless it is capable of 
selecting a smaller one.

### Identity Support ###

Some protocols are capable of finding a specific node based on a provided 
public key. The search should include the destination public key, a list of 
source private keys, and a list of signatures for their corresponding public 
keys.

If the protocol is capable of this, it must also provide a means of limiting 
what nodes will be accepted for connection requests.

- Promiscuious Identification: a specific node can be found, but host must 
  announce its own public keys and what key it is searching for.
- Covert Identification: a specific node can be found without notifying others 
  about the search, other than that some search is happening.

### Routing Obfuscation ###

Some protocols may conceal the origin and destination of traffic.

## Establishing Connections ##

- Key pairs
	- Signatures for each key pair
	- Public keys or signature chains that will allow acknowledgement and 
	  establishment of a connection

