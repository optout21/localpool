# Local Pool

A Bitcoin node add-one for delayed transaction broadcast.

## Modes

### Transparent Mode

Both the bitcoin node and the client software is unmodified.
Delay in the proxy works transparently. The user of the node sees like the regular bitcoin node, delay settings are configured in the proxy.

### Client Mode

TODO
New RPC parameter.
The user of the node has to set it.

## Usage

Assume a local Bitcoin node is available, at "bitcoin.local:8334".

Something like this should work:

```
curl --user myusername --data-binary '{"jsonrpc": "1.0", "id": "curltest", "method": "getblockchaininfo", "params": ["SIGNEDHEXTX"]}' -H 'content-type: text/plain;' http://bitcoin.local:8332/
```

Run LocalPool, connecting it to the bitcoin node. Example:

```
localpool 8432 "http://bitcoin.local:8332/" 30
```

Test it:

```
curl --user myusername --data-binary '{"jsonrpc": "1.0", "id": "curltest", "method": "sendrawtransaction", "params": ["SIGNEDHEXTX"]}' -H 'content-type: text/plain;' http://localhost:8432/
```

This will send a transaction to localpool, localpool will store it, and it will forward it after 30 seconds.


## Misc

Bitcoin Core RPC Documentation:
https://bitcoincore.org/en/doc/30.0.0/
https://developer.bitcoin.org/reference/rpc/

