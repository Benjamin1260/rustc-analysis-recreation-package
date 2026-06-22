# Categorization


## **concurrent (pure) (Avoid Blocking)**
- the code simply shouldn't freeze on the user/ block up
- only **one** thread is every spawned
- don't care if it's interleaved (concurrent)

  ### **io** (tokio)
  - the blocking is caused by some external input/output

    #### fs (tokio)
    - input/output concerning files stored on this device's filesystem

    #### net (tokio)
    - input/output through network with client/server/machine on other device
    - other device in this case can be a VM on local machine

    #### signal (tokio)
    - input/output from external event (e.g. keypress)

  ### time (tokio)
  - the blocking is caused by a timer

## **concurrent_parallel/computation**
- code should not block and needs to be fast (parallel)
- **we do not spawn threads/processes actively** but multiple might be created **under the hood**
- we do not care if ran concurrently/parallel
- computation is blocking &rarr; needs concurrency
- computation benefits from speedup &rarr; needs parallelism
- main bottleneck will be logic/arithmati

  ### blockchain
  - computation/logic is happening on a blockchain

  ### local_service
  - computation/logic is happening inside different process
  - comp/logic should not block
  - comp/logic should be fast
  - requests are sent locally to other process

    #### db
    - local_service is database

    #### testing
    - local_service performs some kind of testing

## **parallel (pure) (speedup)**
- code must run faster by doing multiple 'other things' at once
- **one or more** threads/processes are **actively** intentionally being spawned
- it's important it is ran at same time (parallel)

  ### process (tokio)
  - 'other thing being run' is a different process (not thread)
  - we manage process

  ### sync (tokio)
  - synchronisation/communication/sharing between 'other things'/'threads'
  - things like mutexes, locks, barriers, 

  ### task (tokio)
  - 'other thing being run' is same process, different thread
  - we manage thread

