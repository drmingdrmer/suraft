# suRaft: A Raft Consensus Implementation on Shared Storage


## Overview

**suRaft** is a distributed consensus project that adapts the Raft algorithm to work with shared storage systems like Amazon S3. By leveraging shared storage, `suRaft` simplifies distributed consensus by eliminating log replication, making it ideal for cloud environments with reliable storage services.


## Goals

2. **Cloud-Native Integration**
   - Leverage cloud storage for enhanced reliability and scalability

1. **Simple Consensus**
   - Eliminate log replication processes
   - Minimize coordination overhead

3. **High Throughput and High Concurrency**
   - Optimize performance through efficient use of shared storage
   - Scale write operations effectively


## Non-Goals

1. **NON-GOAL: Storage System Features**
   - Not a full storage system, only a consensus layer
   - Not managing local storage operations
   - Not providing unified access across storage providers(provided by [Apache OpenDAL](https://github.com/apache/opendal))

2. **NON-GOAL: Low Latency Operations**
   - Not optimized for high-frequency updates


## For Whom

1. **Meta-Service for Shared Storage Applications**
   - For applications using shared storage services (e.g., Amazon S3)

   **Benefits:**
   - Atomic transactions and strong consistency for application data
   - Seamless integration with existing cloud infrastructure
   - Simple operations with built-in reliability
