# suRaft: A Raft Consensus Implementation on Shared Storage


[![Discord Chat](https://img.shields.io/discord/1306128478331998298?logo=discord)](https://discord.gg/dPDDCyytkN)

**suRaft** is a distributed consensus project that adapts the Raft algorithm to work with shared storage systems like Amazon S3. By leveraging shared storage, `suRaft` simplifies distributed consensus by eliminating log replication, making it ideal for cloud environments with reliable storage services.

- [Goals and Non-Goals](rfcs/0001-goals.md)
- [Architectural Design](rfcs/0002-arch.md)
- [Data Storage](rfcs/0003-data-storage.md)
- [Server State](rfcs/0004-server-state.md)
- [Protocol](rfcs/0005-protocol.md)
- [Implementaion API](rfcs/0006-implementation-api.md)


Optionally Support:

- [Multi-Column Multi-Leader](rfcs/0099-multi-leader.md)


## Roadmap

- [x] **System Design Docs**

- **Initial Implementation**
    - Leader election
    - Cluster bootstrap
    - Basic log operations
    - Member management


- **Core Features**
    - State Machine Interface
    - Storage cleanup
    - Read path optimization

- **Production Ready**
    - Metrics and monitoring
    - Performance testing
    - Failure testing
    - Documentation

- **Extensions**
    - Storage providers
    - Client libraries
    - Config management
