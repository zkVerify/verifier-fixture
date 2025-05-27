# Replace Native Verifier by Bloom Filter

In this project, we show how replace a native verifier with a compact
structure that can replicate the verifier behavior for all the proofs
processed in the chain history for this verifier.

The project is structured like follow:

1. `verifier-fixture` crate contains the only dependency tha should be imported 
  in the relay chain sources and provide the struct that can be built from binary data.
  In the tests provided in this crate you can find an example of how to instantiate the
  Fixture and use it to verify the proofs.
2. `build-filter-example` crate is a main program with a minimal CLI interface that 
   1. fetch ultraplonk proofs
   2. create a filter
   3. serialize the filter
   4. deserialize the filter
   5. verify again all the proofs by using the filter 
 
3. `helpers` crate that contains all the function that are used by the
  `build-filter-example` to fetch proof data. In general, to create a filter 
  is not necessary to change this code but if you need to access to other
  vk storage you need also add the desired pallet in the `verifiers.scale`
  file. [See at Download chain scale file](#download-chain-scale-file) for more details.

The `build-filter-example` program can be used as a template to create the filter data and save it. What
you should provide is an implementation for the verifier that should cover the json
deserialization and how to call the verify function. There will be some boilerplate
that you should fix about types, but I chose to not make it generic and abstract
to avoid unnecessary complications that could make it harder to understand.


## Download chain scale file

We use `subxt` to interact with a zkverify node. In `helpers/schemas/verifiers.scale` we 
download the scale definition and in `helpers::zkverify_node::zkverify` we use `subxt`
macro to build all the stuff needed to interact with. At the time we were writing we downloaded 
just `SettlementUltraplonkPallet` verifier information, but if you need to implement another
verifier pallet your code should know how to interact with its `Vks` storage.

After installed `subxt` cli (look to `subxt` docs for more information) the following line is 
the one we use to download all we needed. We chose to select just these pallets to reduce
the compile time, add just your pallet definition.

```sh
subxt metadata --url wss://volta-rpc.zkverify.io \
  -o helpers/schemas/verifiers.scale \
  --pallets SettlementUltraplonkPallet,System
```

## Download subsquid graphql scheme

Maybe we'll never need it, but it's always good to know what the `helpers/schemas/zkverify.graphql`
is and how to download it. Like the name says it's the GraphQL scheme of the service
that we use to get all the extrinsic ID, the `(block,id)` tuple, where a `submit_proof` for a given
verifier was submitted.

To download the schema we used the `cynic` CLI (see `cynic` docs for more details and how to install it)

```shell
cynic introspect http://localhost:4350/graphql -o helpers/schemas/zkverify.graphql
```

Our squids service provides also a playground at https://zkpvnetwork.squids.live/zkverify@v3/api/graphiql
(pay attention to the `i` in `graphiql`) and cynic provides also a playground
to write the rust code to build the query at https://generator.cynic-rs.dev/ .
