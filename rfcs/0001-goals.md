# suraft: A Raft Consensus Implementation on Shared Storage

## Overview

**suraft** is a distributed consensus project that adapts the Raft algorithm to work with shared storage systems like Amazon S3. By leveraging shared storage, `suraft` simplifies distributed consensus by eliminating log replication, making it ideal for cloud environments with reliable storage services.

## Goals

1. **Simplify Distributed Consensus**
   - Eliminate redundant log replication processes in distributed systems
   
2. **Cloud-Native Design**
   - Leverage cloud storage infrastructure for enhanced reliability and scalability
   
3. **Easy Integration**
   - Enable seamless deployment across cloud environments with minimal configuration
   
4. **High Throughput**
   - Optimize performance through efficient use of shared storage and minimal coordination

## Non-Goals

1. **NON-GOAL: Complete Storage Solution**
   - Not a full storage system, only a consensus layer

2. **EXCLUDED: Network-Based Replication**
   - Not handling peer-to-peer log replication
   - Not optimizing network communications

3. **NON-GOAL: Local Storage Management**
   - Not managing local storage operations

4. **OUT OF SCOPE: Cross-Storage Provider Compatibility**
   - Not providing unified access across storage providers

5. **NON-GOAL: Low Latency Operations**
   - Not optimized for high-frequency updates

## For Whom

1. **Meta-Service for Shared Storage Applications**
   - For applications using shared storage services (e.g., Amazon S3)
   
   **Benefits:**
   - Atomic transactions and strong consistency for application data
   - Seamless integration with existing cloud infrastructure
   - Simple operations with built-in reliability