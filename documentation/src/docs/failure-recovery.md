---
title: GStreamer Pipeline Failure Recovery
---

<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->

## Transaction Coordinator

The Pravega Transaction Coordinator (pravegatc) element can be used in a pipeline with a pravegasrc element to provide failure recovery.
A pipeline that includes these elements can be restarted after a failure and the pipeline will resume from where it left off.
The current implementation is best-effort which means that some buffers may be processed more than once or never at all.
The pravegatc element periodically writes the PTS of the current buffer to a Pravega table.
When the pravegatc element starts, if it finds a PTS in this Pravega table, it sets the start-timestamp property of the pravegasrc element.

This rest of this document describes the proposed design of a *future* implementation of the Pravega Transaction Coordinator.

First, we will provide AT-LEAST-ONCE only, and a single input/output pad pair.
This may produce duplicate events but this can be bounded by the flush frequency.
Later, we will add the features needed for EXACTLY-ONCE and multiple input/output pad pairs.

### nvmsgbroker (Pravega Event Writer)

This is based on pravega-to-object-detection-to-pravega.py.

```
pravegasrc -> ...nvinfer... -> nvmsgconv -> transactioncoordinator -> nvmsgbroker
```

### pravegasink (Pravega Byte Stream Writer)

```
pravegasrc -> ...nvinfer... -> nvdsosd -> x264enc -> mpegtsmux -> transactioncoordinator -> pravegasink
```

Unlike the event writer, we can easily re-read the data written to the destination stream because it will be in a stream by itself.
However, using this ability would make failure recovery difficult.
Instead, we will assume that we can use a transaction to write to the Pravega byte stream.
This is likely possible since pravegasrc and pravegasink use an event encoding that is compatible with the Pravega event stream writer and reader.
With this assumption, failure recovery of pravegasink becomes the same as nvmsgbroker.

### Multiple Inputs and Outputs

It is also possible that we want to write both the metadata and video data to Pravega exactly-once.

```
pravegasrc -> ...nvinfer... -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker
                             \ nvdsosd -> x264enc -> mpegtsmux -/                  \ \--- pravegasink
                                                                                    \---- pravegasink
```

Multiple pravegasrc can be combined in a single pipeline for the sole purpose of batch processing in the GPU.
Each section of the pipeline is independent except at `nvstreammux -> nvinfer -> nvstreamdemux` where they must be combined.
These can use independent transaction coordinators and they can have independent PTS.

```
pravegasrc A -> ...nvstreammux -> nvinfer -> nvstreamdemux -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker A
                    /                                   \   \ nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink A
pravegasrc B -> .../                                     \--- nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker B
                                                          \-- nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink B
```

It is also possible that we want to perform inference on multiple video frames and produce an output.
This might be useful if the video feeds are cameras pointing at the same area from different angles (or 3D cameras), and we want to build a 3D model.

```
pravegasrc L -> ...nvstreammux -> nvinfer -> nvstreamdemux -> nvmsgconv ---------------------> transactioncoordinator -> nvmsgbroker L+R
                    /                                       \ nvdsosd -> x264enc -> mpegtsmux -/                    \--- pravegasink L+R
pravegasrc R -> .../
```

### Implementation

- In-memory State:
  - pts:
    - u64 which will equal the minimum PTS across inputs
  - active_transactions:
    - (future) list of active transactions
  - ready_to_flush:
    - ordered blocking queue of (`pts`, (future) `transactions`)
    - Events written to the transactions will have a timestamp strictly less than `pts`.

#### Chain function

Below describes the chain function in the Transaction Coordinator (TC).

- (future) Buffers from inputs will be queued (or inputs blocked) as needed to ensure that all buffers are processed in PTS order.
- Calculate `new_pts` = minimum PTS across all inputs.
- If `new_pts` is greater than `pts`.
  - Set `pts_changed` = true.
  - Set `pts` = `new_pts`.
- Determine when prior open transaction should be committed.
  This should be at a frame boundary, or equivalently `pts_changed` is true.
  We can also require the PTS to change by a minimum amount.
- If we should commit:
  - Add record to `ready_to_flush`:
    - `pts`: from new buffer
    - (future) `transactions`: from `active_transactions`
  - (future) Empty `active_transactions`.
  - (future) Begin new transactions and populate `active_transactions`.
  - (future) Notify each output to flush any internal buffers and use the new transactions.
    There is no need to flush the Pravega transactions at this point.
    - nvmsgbroker
      - Send custom event to use the new transaction.
    - pravegasink
      - Send custom event or buffer to use the new transactions (1 for data, 1 for index).
- Chain to outputs.
  - Write to Pravega asynchronously or synchronously.

### Commit thread

- Persistent State:
  - list of (`pts`, (future) `transaction_ids`)
  - This record indicates that we can recover a failed pipeline by commiting `transaction_ids` and then seeking to `pts`.
    A video decoder will need to clip its output to ensure that the first buffer has a PTS equal or greater than `pts`.

This thread will run in the background.

- Repeat forever:
  - Perform failure recovery if previous iteration did not succeed (but only seek the first time).
  - Read a record (`pts`, `transactions`) from the queue `ready_to_flush`.
  - Flush all transactions.
  - Atomically update the persistent state by appending the record (`pts`, (future) `transactions_ids`).
  - (future) Commit all transactions.
  - (future) Atomically update the persistent state by updating the record to have an empty list of `transaction_ids`.
    This avoids problems with committed transactions that expire before the pipeline runs again.

### Failure recovery

- Determine last recorded persistent state.
- (future) For each record (`pts`, `transactions_ids`):
  - Commit all transactions.
- Seek all inputs to `pts`.
  - pravegasrc will find the random access point at or immediately before `pts`.
  - Video decoders must clip output at exact `pts`.
  - Video encoders will start encoding at exact `pts`.
  - Can TC element perform the seek?
