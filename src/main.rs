extern crate printwrap;
use tokio::{process::Command};
use tokio::io::{BufReader,AsyncBufReadExt};
use serde::{Deserialize, Serialize};
use log::*;
use std::{process,env,fs::File,path::Path, process::Stdio,str};
use chrono::{Local};

#[derive(Serialize, Deserialize)]
struct Job 
{
	sourcedataset: String,
	prefix: Option<String>,
	recursive: bool,
	targetdataset: String,
}
#[derive(Serialize, Deserialize)]
struct Piper
{
	jobs: Vec<Job>,
}

fn load_config(file_path: &Path) -> Piper
{
	debug!("Json_file_path: \"{}\"", file_path.display());
	let file = match File::open(file_path) 
	{
		Err(error) => {error!("Could not open file \"{}\"\n{}", file_path.display(),error);process::exit(1)},
		Ok(file) => file,
	};
	let piper= match serde_json::from_reader(file)
	{
		Err(error) => {error!("Error reading file \"{}\"\n{}",file_path.display(), error);process::exit(3)},
		Ok(piper) => piper,
	};
	return piper;
}

fn walk_json(piper: &Piper)
{
	for j in &piper.jobs
	{
		println!("Job:");
		println!("\tSource Dataset:\"{}\"", j.sourcedataset);
		match &j.prefix
		{
			None=>println!("\t\tPrefix: NO-PREFIX"),
			Some(s)=>println!("\t\tPrefix: \"{}\"",s),
		};
		println!("\tRecursive:\"{}\"", j.recursive);
		println!("\tTarget Dataset:\"{}\"", j.targetdataset);
	}
}

fn usage()
{
	printwrap::print_wrap(5,0,"Usage:");
	printwrap::print_wrap(5,0,"    piper [options]");
	printwrap::print_wrap(5,0,"Options:");
	printwrap::print_wrap(5,24,"    -f <config file>    Load the specified JSON config file.");
	printwrap::print_wrap(5,24,"                        Default: /usr/local/etc/znappr/piper.json");
	//printwrap::print_wrap(0,24,"    -s | --stdout       Log messages to stdout rather than syslog.");
	printwrap::print_wrap(5,24,"    -c | --configtest   Validate the config json file then exit.");
	printwrap::print_wrap(5,24,"    -h | --help         Print this usage information and exit.");
	printwrap::print_wrap(5,24,"    -n | -nn            Do a No-Operation dry-run. Performs all actions, except no actual replication will occur. If the \"-n\" option is specified, the zfs send action will include the \"-n\" option and no data will be sent. If the \"-nn\" option is specified, data *will* be sent but the zfs receive action will include the \"-n\" option and no data will be written.");
	printwrap::print_wrap(5,24,"    -p                  Print a generic configuration file. This file will not be tailored to this computer, but will serve as a starting point to customizing your own configuration file.");
	printwrap::print_wrap(5,24,"    -v | -vv            Increase the level of messaging by one or two levels (the maximum).");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"Piper is a zfs replication manager. It is intended to be an accessory to znappr, but can run in conjunction with any other zfs snapshotting scheme.");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"Piper reads a simple config file in /usr/local/etc/znappr/piper.json consisting of a number of jobs each specifying a source dataset, and a target dataset to replicate the source dataset into. Both the target and source can be either local or on remote hosts. For remote hosts, prepend \"<hostname>:\" to the source/target dataset field in the configuration (use the \"-p\" option to print a sample configuration with an example).");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"For each job, piper will, if no replication of that dataset to the target has yet occured, send the full dataset to the destination via zfs send and recieve. If a previous replication already happened, piper will send an incremental update between the most recent snapshot of the dataset, and the previous snapshot replicated.");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"For all actions on remote hosts, the only transport supported is ssh. Keys must have already been created on the local system for the user piper will run as, and copied to the target hosts or the ssh connections will fail and no replication will take place. Remote ssh connections will only be as the same user piper is running as locally. Ssh is used for \"zfs receive\" as well as \"zfs list\" for querying the status of target datasets and past replciation.");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"Piper makes some assumptions and has some defaults:");
	printwrap::print_wrap(5,8,"  - Replication will always include the \"-R\" and \"-s\" zfs send options. The -s option will skip child datasets IF the child does not have the same most recent snapshot name. E.g. if the replicated dataset has a snapshot @DAY___2025-05-19 but the child's most recent snapshot is @DAY___2025-05-11 then the child will be skipped. Errors will be logged, but the replication of the parent will still take place.");
	printwrap::print_wrap(5,8,"  - If the source dataset is encrypted, the \"-w\" (raw) option will be used.");
	printwrap::print_wrap(5,8,"  - canmount will be set to off (\"-o canmount=none\") on zfs recv for all replications.");
	printwrap::print_wrap(5,8,"  - The zfs receive will include \"-F\" (force rollback/purge).");
	printwrap::print_wrap(5,8,"  - Piper does not create snapshots, but at least one snapshot must exist in order to replicate a dataset. At least a second must exist in the source dataset and the first in both the source and destination datasets to perform an incremental replication. Piper will inspect the source and destination datasets to determine which snapshots to be used by using zfs list and sorting by the createtxg property. The source dataset must be local, but the destination dataset may be on a remote host indicated by prepending the \"<hostname>:\" to the target dataset name in the configuration.");
	printwrap::print_wrap(5,8,"  - Piper does not care where these snapshots came from, but if the last snapshot used for replication is destroyed, further replication attempts will fail as incremential replication is always between a current snapshot the previous snapshot used. If that snapshot doesn't exist, it can't be used as a base for further replication.");
	printwrap::print_wrap(5,8,"  - Piper does not destroy snapshots on the source, either, but the \"-F\" option on zfs receive does have the side effect/benefit of purging snapshots on the destination that no-longer exist on the source.");
	printwrap::print_wrap(5,8,"  - Piper by default will replicate the first snapshot found for a given dataset. Sometimes this may not be desired. If one makes snapshots every 5 minutes *and* every hour, but purge the 5-minute snapshots after 2 hours, an initial replication at midnight may replicate the most recent 5-minute snapshot. However, an incremental replication the following night will attempt to perform an incremental between the current most recent 5-minute snapshot and the 5-minute snapshot from the previous night ... which would have been purged. This replication will fail. To avoid this, an optional field labeled \"prefix\" can be included in the configuration file. Piper will *only* replicate snapshots with this string at the beginning of the snapshot tag. For example, a configuration file with the line:");
	printwrap::print_wrap(5,8,"              \"prefix\" : \"HOURLY__\",");
	printwrap::print_wrap(5,8,"        for the replication job will only replicate snapshots which begin with \"HOURLY__\", and ignore all others. If no other snapshots exist, replication will not happen.");
	printwrap::print_wrap(5,8,"");
	printwrap::print_wrap(5,0,"All piper logging is to stdout.");
	printwrap::print_wrap(5,0,"");
	printwrap::print_wrap(5,0,"Piper is intended to be run via cron. When running from cron, the frequency piper is run should correspond to the most frequent snapshots for each dataset to be replicated. Run piper a few minutes after the snapshots are scheduled. If you make daily snapshots there is no need to run piper more frequently. It won't hurt, but isn't necessary. A typical cron line for daily replication might look like this:");
	printwrap::print_wrap(5,0,"    5  0  *  *  *    /usr/local/bin/piper  &>> /var/log/piper.log");
	printwrap::print_wrap(5,8,"or, for hourly replication:");
	printwrap::print_wrap(5,0,"    5  *  *  *  *    /usr/local/bin/piper  &>> /var/log/piper.log");
	printwrap::print_wrap(5,8,"");

	process::exit(1);
}

fn print_config()
{
	println!("{{\n\t\"comment\":\"piper configuration.\",\n\t\"jobs\": [\n\t\t\t{{\n\t\t\t\t\"sourcedataset\" : \"zroot/ROOT/root\",\n\t\t\t\t\"recursive\" : true,\n\t\t\t\t\"targetdataset\": \"remoteserver:zroot/backups/computer\"\n\t\t\t}},\n\t\t\t{{\n\t\t\t\t\"sourcedataset\" : \"zroot/home\",\n\t\t\t\t\"recursive\" : true,\n\t\t\t\t\"targetdataset\": \"remoteserver:zroot/backups/computer\"\n\t\t\t}}\n\t]\n}}");
	process::exit(1);
}

fn can_login_to_host(host:&str) -> bool
{
	let mut can_login_status=false;
	info!("Can log into host \"{}\"", host);
	debug!("testing \"ssh {} exit\"",host);
	let mut can_login = std::process::Command::new("ssh");
			can_login.arg(host);
			can_login.arg("exit");
	let can_login_out= match can_login.stderr(Stdio::piped())
			.output()
			{
				Err(e)=>{error!("Error getting error output from can_login:{}",e);return can_login_status},
				Ok(can_login_out)=>can_login_out
			};
	let stderr = match String::from_utf8(can_login_out.stderr)
			{
				Err(e)=>{error!("Error converting can_login_out to utf8:{}",e);return can_login_status},
				Ok(stderr)=>stderr
			};

	match can_login.status()
	{
		Err(_e)=> can_login_status=false,
		Ok(o)=> can_login_status=o.success()
	}
	if !can_login_status
	{
		let lines = stderr.lines();
		for line in lines
		{
			error!("{}",line);
		}
	}
	debug!("Can_login_to_host: \"{}\"", can_login_status);
	return can_login_status;
}

//is_dataset_encrypted needs to return a Result<T> as bool isn't sufficient (i.e. results need to be true,false,error)
fn is_dataset_encrypted(dataset:&str) -> bool
{
	info!("is dataset encrypted {}", dataset);
	let full_command = format!("zfs list -H -t filesystem -o encryption  {}",dataset);
	debug!("{}",full_command);
	let fs_list = match std::process::Command::new("zfs")
			.arg("list")
			.arg("-H")
			.arg("-t")
			.arg("filesystem")
			.arg("-o")
			.arg("encryption")
			.arg(dataset)
			.stdout(Stdio::piped())
			.output()
			{
				Err(e)=>{error!("Error on \"{}\":{}",full_command,e);return false},// this isn't good enough
				Ok(fs_list)=>fs_list,
			};
	let stdout = match String::from_utf8(fs_list.stdout)
				{
					Err(e)=> {error!("error converting fs_list output to utf8: {}",e);return false},
					Ok(stdout)=>stdout,
				};
	let mut lines = stdout.lines();
	let line = String::from(match lines.next()
								{
									Some(text)=> text,
									None=>"",
								});
	let is_encrypted = if line == "off" { false} else {true};

	return is_encrypted;
}

fn get_child_datasets(dataset:&str) -> String
{
	let error_value=String::from("");
	info!("get child datasets \"{}\"", dataset);
	debug!("zfs list -H -d 2 -t filesystem -o name {}",dataset);
	let mut snapshot_list = std::process::Command::new("zfs");
			snapshot_list.arg("list");
			snapshot_list.arg("-H");
			snapshot_list.arg("-d");
			snapshot_list.arg("1");
			snapshot_list.arg("-t");
			snapshot_list.arg("filesystem");
			snapshot_list.arg("-o");
			snapshot_list.arg("name");
			snapshot_list.arg(dataset);
	let snapshot_output= match snapshot_list.stdout(Stdio::piped())
			.output()
			{
				Err(e)=>{error!("Error getting child datasets output:{}",e);return error_value },
				Ok(snapshot_output)=>snapshot_output,
			};
	let stdout = match String::from_utf8(snapshot_output.stdout)
			{
				Err(e)=>{error!("Error converting child datasets output to uft8:{}",e);return error_value },
				Ok(stdout)=>stdout,
			};
	return stdout
}

fn rsplit_once(source:&str, split_on:char) -> String
{
	let (_,value) = match source.rsplit_once(split_on)
					{
						None=> ("",""),
						Some(value)=> value,
					};
	String::from(value)
}

// probably need Result<T> here as well: true,false,error
fn does_dataset_exist_on_target(sourcedataset:&str, targetdataset:&str, host:&str) -> bool
{
	let mut dataset_exists=false;
	info!("does source dataset ({}) exist in \"{}\" on \"{}\"", sourcedataset, targetdataset, host);
	let datasetname = rsplit_once(sourcedataset, '/');
	let targetdatasetname = format!("{}/{}", targetdataset,datasetname);
	let ssh = if host=="" {String::from("")}else{format!("ssh {} ", host)};
	debug!("{}zfs list -H -t filesystem -o name -S createtxg {}",ssh,targetdatasetname);
	let mut dataset_list = if host=="" {std::process::Command::new("zfs")}else{std::process::Command::new("ssh")};
			if host != ""
			{
				dataset_list.arg(host);
				dataset_list.arg("zfs");
			}
			dataset_list.arg("list");
			dataset_list.arg("-H");
			dataset_list.arg("-t");
			dataset_list.arg("filesystem");
			dataset_list.arg("-o");
			dataset_list.arg("name");
			dataset_list.arg("-S");
			dataset_list.arg("createtxg");
			dataset_list.arg(targetdatasetname);
	let dataset_list_out= match dataset_list.stdout(Stdio::piped())
			.output()
			{
				Err(e)=> {error!("Error getting dataset_list_out:{}",e);return false},
				Ok(dataset_list_out)=>dataset_list_out,
			};
	info!("dataset_list_out status: \"{}\"", match dataset_list.status() { Err(e)=>{format!("{}",e)},Ok(o)=>format!("{}",o)});
	if dataset_list_out.status.success()
	{
		// maybe should be info! rather than debug!
		debug!("Dataset exists on target.");
		dataset_exists = true;
	}
	else
	{
		// maybe should be info! rather than debug!
		debug!("Dataset does not exist on target.");
	}
	return dataset_exists;
}


fn get_last_replicated_snapshot(sourcedataset:&str, targetdataset:&str, host:&str) -> String
{
	let error_value = String::from("");
	info!("get last replicated snapshot named \"{}\" in \"{}\" on \"{}\"", sourcedataset, targetdataset, host);
	let datasetname = rsplit_once(sourcedataset, '/');
	let targetdatasetname = format!("{}/{}", targetdataset,datasetname);
	let ssh = if host=="" {String::from("")}else{format!("ssh {} ", host)};
	debug!("{}zfs list -H -t snapshot -o name -S createtxg {}",ssh,targetdatasetname);
	let mut snapshot_list = if host=="" {std::process::Command::new("zfs")}else{std::process::Command::new("ssh")};
			if host != ""
			{
				snapshot_list.arg(host);
				snapshot_list.arg("zfs");
			}
			snapshot_list.arg("list");
			snapshot_list.arg("-H");
			snapshot_list.arg("-t");
			snapshot_list.arg("snapshot");
			snapshot_list.arg("-o");
			snapshot_list.arg("name");
			snapshot_list.arg("-S");
			snapshot_list.arg("createtxg");
			snapshot_list.arg(targetdatasetname);
	let snapshot_out= match snapshot_list.stdout(Stdio::piped())
			.output()
			{
				Err(e)=> {error!("Error getting snapshot_list stdout {}",e);return error_value},
				Ok(snapshot_out)=>snapshot_out,
			};
	let stdout = match String::from_utf8(snapshot_out.stdout)
						{
							Err(e)=>{error!("Error converting output to UTF8:{}",e);error_value},
							Ok(stdout)=>stdout
						};
	info!("snapshot_out status: \"{}\"", match snapshot_list.status() { Err(e)=>{format!("{}",e)},Ok(o)=>format!("{}",o)});
	if snapshot_out.status.success()
	{
		let line = stdout.lines().next();
		if let None = line
		{
			info!("No last replicated snapshot");
			return String::from("")
		}
		let uline = match line
						{
							None=>"",
							Some(ul)=>ul,
						};
		let name = rsplit_once(uline, '@');
		info!("Last replicated snapshot:\"{}\"", name);
		return String::from(name);
	}
	else
	{
		error!("Dataset does not exist on target. Has not been replicated yet.");
		return String::from("");
	}
}

fn get_most_recent_snapshot(dataset:&str, host:&str, prefix: &str) -> String
{
	let error=String::from("//!!--XX--ERROR--XX--!!\\\\"); 
	info!("get most recent snapshot named \"{}\" on \"{}\"", dataset, host);
	debug!("zfs list -H -t snapshot -o name -S createtxg {}",dataset);

	let mut snapshot_list = if host=="" {std::process::Command::new("zfs")}else{std::process::Command::new("ssh")};
			if host != ""
			{
				snapshot_list.arg(host);
				snapshot_list.arg("zfs");
			}
			snapshot_list.arg("list");
			snapshot_list.arg("-H");
			snapshot_list.arg("-t");
			snapshot_list.arg("snapshot");
			snapshot_list.arg("-o");
			snapshot_list.arg("name");
			snapshot_list.arg("-S");
			snapshot_list.arg("createtxg");
			snapshot_list.arg(dataset);
	let snapshot_out = match snapshot_list.stdout(Stdio::piped())
			.output()
			{
				Err(e)=>{error!("Error getting snapshot_lit output:{}",e);return error},
				Ok(snapshot_out)=>snapshot_out,
			};

	let stdout = match String::from_utf8(snapshot_out.stdout)
				{
					Err(e)=>{error!("Error converting snapshot_out to utf8:{}",e);error},
					Ok(stdout)=>stdout,
				};

	for line in stdout.lines()
	{
		trace!("Examning snapshot \"{}\"", line);
		let name = rsplit_once(line, '@');
		trace!("\t tag \"{}\"", name);
		if prefix == ""
		{
			trace!("\t Prefix is empty, so take this, the first result.");
			return String::from(name);
		}
		else
		{
			trace!("\t Prefix is \"{}\"", prefix);
			if name.starts_with(prefix)
			{
				trace!("\t\tName starts with prefix, so return this result.");
				return String::from(name);
			}
			else
			{
				trace!("\t\tName does NOT start with prefix, on to the next result...");
			}
		}
	}
	return String::from("")
}

fn split_host_and_dataset(string:&str) -> (&str, &str)
{
	let mut host="";
	let dataset;
	let parts = string.split(":");
	let count = parts.collect::<Vec<&str>>();
	if count.len() == 2
	{
		host=count[0];
		dataset=count[1];
	}
	else
	{
		dataset=count[0];
	}
	(host, dataset)
}

async fn process_job(j:&Job, send_no_op:bool, recv_no_op:bool) -> bool
{
	let mut completed=true;
	let (sourcehost,sourcedataset)=split_host_and_dataset(&j.sourcedataset);
	let (targethost,targetdataset)=split_host_and_dataset(&j.targetdataset);
	let recursive=j.recursive;
	let prefix = match &j.prefix
		{
			None=>"",
			Some(s)=>s,
		};

	let encrypted=is_dataset_encrypted(sourcedataset);
	if sourcehost != ""
	{
		info!("sourcehost: \"{}\"", sourcehost);
		if !can_login_to_host(sourcehost)
		{
			error!("Can't replicate: can't login to source host {}.", targethost);
			return false;
		}
	}
	info!("sourcedataset: \"{}\"", sourcedataset);
	info!("recursive: \"{}\"", recursive);
	if targethost != ""
	{
		info!("targethost: \"{}\"", targethost);
		if !can_login_to_host(targethost)
		{
			error!("Can't replicate: can't login to target host {}.", targethost);
			return false;
		}
	}
	info!("targetdataset: \"{}\"", targetdataset);
	info!("encrypted: \"{}\"", encrypted);

	let current_snapshot_name=get_most_recent_snapshot(sourcedataset, sourcehost, prefix);
	let previous_snapshot_name=get_last_replicated_snapshot(sourcedataset, targetdataset, targethost);
	if previous_snapshot_name != ""
	{
		info!("{} exists on target. Dataset has been replicated, so we'll check most recent snapshot.", &j.sourcedataset);
		if  previous_snapshot_name != current_snapshot_name
		{
			info!("\"{}\" != \"{}\"", previous_snapshot_name, current_snapshot_name);
			info!("Snapshots do not match, doing incremental replication.");
			let current_snapshot_name_full = format!("{}@{}", sourcedataset, current_snapshot_name);
			let previous_snapshot_name_full = format!("{}@{}", sourcedataset, previous_snapshot_name);

			if replicate(sourcehost, sourcedataset,current_snapshot_name_full.as_str(), previous_snapshot_name_full.as_str(), encrypted, recursive, targethost, targetdataset, send_no_op, recv_no_op).await
			{
				info!("Incremental Replication succeeded.");
			}
			else
			{
				error!("Incremental Replication failed.");
				completed=false;
			}
		}
		else
		{
			// dataset has been replciated, snapshot's match so no additional replication required no
			info!("Snapshot's match, so no additional replication required now.");
		}
	}
	else
	{
		// do full replication
		// but, first, check whether or not the source dataset exists on the target without any snapshots.
		// we've already established there are not snapshots, but we need to check for the dataset.
		// if the dataset exists without any snapshots, we can't replicate as that would overwrite the
		// existing dataset and zfs recv will not do that.
		if does_dataset_exist_on_target(sourcedataset, targetdataset, targethost)
		{
			// dataset exists on target, but doesn't have any snapshots. can't replicate.
			let datasetname = rsplit_once(sourcedataset, '/');
			let targetdatasetname = format!("{}/{}", targetdataset,datasetname);

			error!("Target dataset exists but has no snapshots. Can't replicate.");
			error!("To \"fix\" this either replicate to another parent dataset, or");
			error!("destroy the target dateset: \"{}\" on {}, and then",targetdatasetname, if targethost==""{"lostalhost"}else{targethost});
			error!("re-reun the replication.");
			error!("!!!! THIS WILL DESTROY DATA !!!!");
			error!("DO NOT DO THIS UNLESS YOU ARE VERY SURE IT IS THE CORRECT ACTION TO TAKE.");
		}
		else
		{
			info!("{} does not exist on target. No replication has occured, full replication commencing.", &j.sourcedataset);
			info!("Last snapshot made: \"{}\"", current_snapshot_name);

			let current_snapshot_name_full = format!("{}@{}", sourcedataset, current_snapshot_name);
			if replicate(sourcehost, sourcedataset,current_snapshot_name_full.as_str(), "", encrypted, recursive, targethost, targetdataset, send_no_op, recv_no_op).await
			{
				info!("Full Replication succeeded.");
			}
			else
			{
				error!("Full Replication failed.");
				completed=false;
			}
		}
	}
	return completed;
}

async fn replicate(sourcehost:&str, sourcedataset:&str, snapshot_name:&str, previous_snapshot_name:&str, 
					encrypted:bool, recursive:bool, targethost:&str, targetdataset:&str, 
					send_no_op:bool, recv_no_op:bool) -> bool
{
	let mut replication_status = false;
	info!("REPLICATE");
	info!("sourcehost            : \"{}\"", sourcehost);
	info!("snapshot_name         : \"{}\"", snapshot_name);
	info!("previous_snapshot_name: \"{}\"", previous_snapshot_name);
	info!("encrypted             : \"{}\"", encrypted);
	info!("recursive             : \"{}\"", recursive);
	info!("targethost            : \"{}\"", targethost);
	info!("targetdataset         : \"{}\"", targetdataset);
	info!("send_no_op            : \"{}\"", send_no_op);
	info!("recv_no_op            : \"{}\"", recv_no_op);

	if snapshot_name == previous_snapshot_name
	{
		// This condition should never happen here as this is checked in process_job prior to
		// calling replicate. But... it doesn't hurt to have it here.
		info!("Current/Previous snapshots are the same. Can't replicate 'cause there's nothing new to replicate.");
	}
	else
	{
		info!("Sending \"{}\":\"{}\" to \"{}\":\"{}\"", sourcehost, snapshot_name, targethost, targetdataset);

		let mut sendc = if sourcehost != "" {Command::new("ssh")} else { Command::new("zfs")};
			if sourcehost != ""
			{
				info!("pull from {}", sourcehost);
				sendc.arg(sourcehost);
				sendc.arg("zfs");
			}
			sendc.arg("send");
			if send_no_op
			{
				sendc.arg("-n");
			}
			if encrypted
			{
				sendc.arg("-w");
			}
			sendc.arg("-R");
			sendc.arg("-s");
			if !recursive
			{
				info!("Replication is not recursive. Checking for child datasets...");
				// if not recursive, we need to exclude child datasets
				let stdout = get_child_datasets(sourcedataset);
				let mut skipped=false;
				let mut excluded=false;
				for line in stdout.lines()
				{
					if skipped
					{
						info!("Excluding child dataset: \"{}\"", line);
						sendc.arg("-X");
						sendc.arg(line);
						excluded = true;
					}
					else
					{
						skipped=true;
					}
				}
				if !excluded
				{
					info!("No child datasets to exclude.");
				}
			}
			if previous_snapshot_name != ""
			{
				sendc.arg("-i");
				sendc.arg(previous_snapshot_name);
			}
			sendc.arg(snapshot_name);
		let mut sendo = match sendc.stdout(Stdio::piped())
			.spawn()
			{
				Err(e)=>{error!("Error getting senc stdout:{}",e);return replication_status},
				Ok(sendo)=>sendo,
			};
		debug!("Created sendc & sendo");

		let recv_stdin_a = match sendo.stdout.take()
								{
									None=>{error!("Failed to take sendo.stdout");return replication_status;},
									Some(o)=>o,
								};
		let recv_stdin: Stdio = match recv_stdin_a.try_into()
									{
										Err(_e)=>{error!("Failed to try_into sendo.stdout");return replication_status;},
										Ok(o)=>o,
									};
		debug!("Created recv_stdin");
		let mut recvc = if targethost != "" { Command::new("ssh")}else{Command::new("zfs")};
			if targethost != ""
			{
				info!("push to {}", targethost);
				recvc.arg(targethost);
				recvc.arg("zfs");
			}
			recvc.arg("recv");
			if send_no_op || recv_no_op
			{
				recvc.arg("-n");
			}
			recvc.arg("-v");
			recvc.arg("-e");
			recvc.arg("-o");
			recvc.arg("canmount=off");
			recvc.arg("-F");
			recvc.arg("-u");
			recvc.arg(targetdataset);
			recvc.stdin(recv_stdin);
		let mut recvo = match recvc.stdout(Stdio::piped())
			.spawn()
			{
				Err(e)=>{error!("Error creating recvo. Replication may have occured: {}",e);return replication_status},
				Ok(recvo)=>recvo,
			};
		debug!("Created recvc & recvo");
		let stdout = match recvo.stdout.take()
								{
									None=>{error!("Failed to take recvo.stdout. Replication may have occured.");return replication_status;},
									Some(o)=>o,
								};
		debug!("got stdout from recvo");
		let (send_output, recv_output) = (sendo.wait_with_output().await, recvo.wait_with_output().await);
		debug!("waited output and have results.");
		let so = match send_output
						{
							Err(e) => {error!("Failed to get send_output. Replication may have occured:{}",e);return replication_status},
							Ok(so)=>so,
						};
		if so.status.success()
		{
			let ro = match recv_output
						{
							Err(e) => {error!("Failed to get recv_output. Send was successful, but receive is unknown. Replication may have occured:{}",e);return replication_status},
							Ok(ro)=>ro,
						};
			if ro.status.success()
			{
				replication_status=true;

				let mut reader = BufReader::new(stdout).lines();
				while let Some(line) = match reader.next_line().await {Ok(l)=>l,Err(e)=>{error!("Replication succeeded, but there was an error reading the results:{}",e);return replication_status} }
				{
					info!("ZFS RECV: {}", line);
				}
			}
			else
			{
				error!("ZFS Receive failed.");
			}
		}
		else
		{
			error!("ZFS Send failed:");
			for x in &so.stderr 
			{
				error!("{x}");
			}
		}
	}
	debug!("REPLICATION Done");
	return replication_status
}

#[tokio::main]
async fn main()
{
	let args: Vec<String> = env::args().collect();
	let start=1;
	let end=args.len();
	let mut verbose = log::Level::Info; // default log level of INFO
	let mut do_walk=false;
	let mut json_file_path = Path::new("/usr/local/etc/znappr/piper.json");
	let mut skip_argument=false;
	let mut send_no_op=false;
	let mut recv_no_op=false;

	for i in start..end
	{
		if skip_argument
		{
			skip_argument = false;
		}
		else
		{
			match args[i].as_ref()
			{
				"-h" | "--help" =>
					{
					usage();
					}
				"-f" =>
					{
						if (i+1) < end
						{
							json_file_path = Path::new(&args[i+1]);
							skip_argument = true;
						}
						else
						{
							error!("No config file on command line.");
						}
					}
				"-n" =>
					{
						send_no_op = true;
					}
				"-nn" =>
					{
						recv_no_op = true;
					}
				"-p" =>
					{
						print_config();
					}
				"-v" =>
					{
						verbose = log::Level::Debug;
					} 
				"-vv" =>
					{
						verbose = log::Level::Trace;
					} 
				"-c"|"--configtest" =>
					{
						do_walk=!do_walk;
					}
				_ =>
					{
						println!("Unknown argument \"{}\".",args[i]);
						usage();
					}

			}
		}
	}

	match stderrlog::new().module(module_path!()).verbosity(verbose).init()
	{
		Err(e) => {println!("Error creating stderrlog:{}",e);process::exit(10)},
		Ok(l)=>l, // don't need to do anything for this case.
	};

	let piper = load_config(json_file_path);
	if do_walk
	{
		walk_json(&piper);
		process::exit(0);
	}

	let start_time = Local::now();
	info!("--------------------------------------------------------------------------------");
	info!("{}", start_time);
	info!("Piper Beginning Replication Jobs");
	for j in &piper.jobs
	{
		process_job(&j, send_no_op, recv_no_op).await;
	} 
	let end_time = Local::now();
	info!("Piper Ending Replication Jobs");
	info!("{}", end_time);
}
