# Multi-Column Logs and Multi-Leader Support

**suRaft** optionally supports working with multiple log columns and multiple leaders.
## Advantages, Limitations, and Use Cases

A multi-leader setup offers the advantage that writes on each leader do not block one another, resulting in increased write throughput and reduced latency. However, the drawback is that there is no strict guarantee of linearizability between different columns (leaders). Therefore, this configuration is useful in scenarios where transactions are not strictly dependent on each other.

One practical example is a cross-data-center deployment. In such a setup, each data center has its own leader and associated log column. Data synchronization between data centers occurs in a relatively lazy manner, which is acceptable because strict linearizability across the entire system is not required. This approach allows each data center to operate independently while still maintaining eventual consistency across the distributed system. Furthermore, when required, strict **causal** ordering can also be enforced.
## Establishing Log Entry Ordering

In a multi-column log setup, each leader writes to a specific log column. Since log entries in different columns do not have a defined order relative to each other, we need a way to establish a partial order so that these entries can be applied to the state machine in a consistent and deterministic manner.

## Defining Partial Order with Vector Clocks

We use **vector clocks** to define the partial ordering of log entries. In this context, each component of the vector clock corresponds to the `LogIndex` from a log column.

The partial order between two vector clocks `a` and `b` is defined as follows:

- **Greater Than or Equal (`≥`)**: Vector clock `a` is greater than or equal to vector clock `b` if and only if each component of `a` is greater than or equal to the corresponding component in `b`. Formally:

  ```
  a ≥ b iff for all i, a_i ≥ b_i
  ```

When a log entry is appended to a log column, its vector clock is constructed by taking the union of the vector clocks of the last log entries in each column, updated with its own `LogIndex`.
### Benefits of This Approach

- **Linearization Within Columns**: Log entries within each column are linearized. A newer log entry in a column always has a greater vector clock than the previous entries in that column.

- **Unique Vector Clocks**: Each log entry has a unique vector clock, ensuring a distinct position in the partial order.

## Handling Log Appends Without Up-to-Date Vector Clocks

A leader does not necessarily need to read the most recent vector clocks from all columns when appending a new log entry. If the leader lacks the latest information, it can build a vector clock based on its current knowledge. This vector clock may be relatively smaller, and the entry might be applied earlier.

However, there may be cases where a log entry is committed without the leader being aware of newer entries from other columns. This scenario can break linearizability concerning wall-clock time. To maintain strict linearizability, clients should explicitly provide a vector clock to ensure that the new log entry's vector clock is sufficiently advanced.

## Applying Log Entries to the State Machine

The vector clocks establish a partial order among log entries. When applying these entries to the state machine, the implementation must ensure:

- **Consistent Total Order**: Log entries are applied in a total order that is consistent across all replicas and remains the same in every execution, even when multiple state machines are involved.

One method to derive a total order from the partial order is to compute the dot product of each vector clock with a vector of ones (e.g., `[1, 1, 1, ...]`). This effectively sums the components of the vector clock, providing a scalar value that can be used to order the log entries. (And if two vector clock yield the same product, determine the order by their column index)


## Example of Multi-Leader Log Entries with Vector Clocks

Consider a system with **three leaders** A, B, and C, each writing to its own log column.

### Log Entries and Their Vector Clocks

1. **Leader A, B, C** append two entries without noticing each other:

   ```
   Column 1: [E₁₁, 100], [E₁₂, 200]
   Column 2: [E₂₁, 010], [E₂₂, 020]
   Column 3: [E₃₁, 001], [E₃₂, 002]
   ```

2.   **Leader A** appends `E₁₃` without knowledge of others, vector clock `300`. **Leader B** appends `E₂₃` concurrently, vector clock `030`.

   ```
   Column 1: [E₁₁, 100], [E₁₂, 200], [E₁₃, 300]
   Column 2: [E₂₁, 010], [E₂₂, 020], [E₂₃, 030]
   Column 3: [E₃₁, 001], [E₃₂, 002]
   ```

3.  **Leader C** becomes aware of `E₁₃` and `E₂₃` and appends `E₃₃`:   `E₁₃`: `300`   `E₂₃`: `030`   `E₃₂`: `002`.  Takes the component-wise maximum and increments its own component: Vector clock for `E₃₃`: `333`.

   ```
   Column 1: [E₁₁, 100], [E₁₂, 200], [E₁₃, 300]
   Column 2: [E₂₁, 010], [E₂₂, 020], [E₂₃, 030]
   Column 3: [E₃₁, 001], [E₃₂, 002], [E₃₃, 333]
   ```

5.  **Leader A** appends `E₁₄` after learning about :  `E₁₃`: `300`  `E₃₃`: `333`  Takes the component-wise maximum and increments its own component: Vector clock for `E₁₄`: `433`.

   ```
   Column 1: [E₁₁, 100], [E₁₂, 200], [E₁₃, 300], [E₁₄, 433]
   Column 2: [E₂₁, 010], [E₂₂, 020], [E₂₃, 030]
   Column 3: [E₃₁, 001], [E₃₂, 002], [E₃₃, 333]
   ```

### Establishing Order

- **Within Each Column**:  totally ordered by their vector clocks.  For **Column 1**:

    ```
    E₁₁ (100) < E₁₂ (200) < E₁₃ (300) < E₁₄ (433)
    ```

- **Across Columns**:  **Comparable Entries**:

    - `E₃₃` (`333`) happens after both `E₁₃` (`300`) and `E₂₃` (`030`), because `333` is component-wise greater than or equal to both `300` and `030`.
    - `E₁₄` (`433`) happens after `E₃₃` (`333`).

- **Across Columns**:  **Concurrent Entries**:

    - `E₁₃` (`300`) and `E₂₃` (`030`) are concurrent; their vector clocks are incomparable since neither is greater in all components.

### Applying Log Entries

To ensure consistent application across all replicas, we establish a **total order** using scalar values derived from the vector clocks.

1. **Compute Scalar Values (Sum of Digits)**:

   | Entry  | Vector Clock | Scalar Sum | Column Id |
   |--------|--------------|------------|-----------|
   | `E₁₁`  | `100`        | 1          | 1         |
   | `E₂₁`  | `010`        | 1          | 2         |
   | `E₃₁`  | `001`        | 1          | 3         |
   | `E₁₂`  | `200`        | 2          | 1         |
   | `E₂₂`  | `020`        | 2          | 2         |
   | `E₃₂`  | `002`        | 2          | 3         |
   | `E₁₃`  | `300`        | 3          | 1         |
   | `E₂₃`  | `030`        | 3          | 2         |
   | `E₃₃`  | `333`        | 9          | 3         |
   | `E₁₄`  | `433`        | 10         | 1         |

2.  **Order entries by scalar sum and column id** in ascending order.

   ```
   Column 1: E₁₁,100  .→ E₁₂,200  .→ E₁₃,300  .→ E₁₄, 433
              ↓       |   ↓       |   ↓       |
   Column 2: E₂₁,010  |  E₂₂,020  |  E₂₃,030  |
              ↓       |   ↓       |   ↓       |
   Column 3: E₃₁,001--'  E₃₂,002--'  E₃₃,333--'
   ```
