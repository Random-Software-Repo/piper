# piper

Piper is a zfs replication manager. It is intended to be an accessory to 
znappr, but can run in conjunction with any other zfs snapshotting scheme.

Piper reads a simple config file in /usr/local/etc/znappr/piper.json 
consisting of a number of jobs each specifying a source dataset, and a 
target dataset to replicate the source dataset into. Both the target and 
source can be either local or on remote hosts. For remote hosts, prepend 
"<hostname>:" to the source/target dataset field in the configuration. Use
the "-p" option with piper to print a sample configuration file.

For each job, piper will, if no replication of that dataset to the target 
has yet occured, send the full dataset to the destination via zfs send and 
recieve. If a previous replication already happened, piper will send an 
incremental update between the most recent snapshot of the dataset, and the 
previous snapshot replicated.

For all actions on remote hosts, the only transport supported is ssh. Keys 
must have already been created on the local system for the user piper will 
run as, and copied to the target hosts or the ssh connections will fail and 
no replication will take place. Remote ssh connections will only be as the 
same user piper is running as locally. Ssh is used for "zfs receive" as well 
as "zfs list" for querying the status of target datasets and past replciation.


## Piper makes some assumptions and has some default behavior

-  Replication will always include the "-R" and "-s" zfs send options. The -s option will skip child datasets IF the child does not have the same most recent snapshot name. E.g. if the replicated dataset has a snapshot @DAY___2025-05-19 but the child's most recent snapshot is @DAY___2025-05-11 then the child will be skipped. Errors will be logged, but the replication of the parent will still take place.
-  If the source dataset is encrypted, the "-w" (raw) option will be used.
-  canmount will be set to off ("-o canmount=none") on zfs recv for all replications.
-  The zfs receive will include "-F" (force rollback/purge).
-  Piper does not create snapshots, but at least one snapshot must exist in order to replicate a dataset. At least a second must exist in the source dataset and the first in both the source and destination datasets to perform an incremental replication. Piper will inspect the source and destination datasets to determine which snapshots to be used by using zfs list and sorting by the createtxg property. The source dataset must be local, but the destination dataset may be on a remote host indicated by prepending the "<hostname>:" to the target dataset name in the configuration.
-  Piper does not care where these snapshots came from, but if the last snapshot used for replication is destroyed, further replication attempts will fail as incremential replication is always between a current snapshot the previous snapshot used. If that snapshot doesn't exist, it can't be used as a base for further replication.
-  Piper does not destroy snapshots on the source, either, but the "-F" option on zfs receive does have the side effect/benefit of purging snapshots on the destination that no-longer exist on the source.

## Building:
### Prerequisites:
-  Rust 1.63 or newer
-  Both the piper and printwrap repos from random-software-repo:
```
git clone https://www.github.com/Random-Software-Repo/piper
git clone https://www.github.com/Random-Software-Repo/printwrap
````
-  Gnu Make (make on most if not all linux distrobutions, gmake on FreeBSD)
     
### To compile:
  Run `make build` or `cargo build --release`

### To install:
  Run `sudo make install` or `sudo make install dir=/an/alternate/path/for/piper`

## Running

Piper is intended to be run via cron. When running from cron, the frequency 
piper is run should correspond to the most frequent snapshots for each 
dataset to be replicated. Run piper a few minutes after the snapshots are 
scheduled. If you make daily snapshots there is no need to run piper more 
frequently. It won't hurt, but isn't necessary. 

A typical cron line for daily replication might look like this:
```
5  0  *  *  *    /usr/local/bin/piper  >> /var/log/piper.log 2>&1
```
or, for hourly replication:
```
5  *  *  *  *    /usr/local/bin/piper  >> /var/log/piper.log 2>&1
```