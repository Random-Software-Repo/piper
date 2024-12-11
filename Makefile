
CXX = cargo build --release --target-dir target

PROGRAM = piper

default:
	@echo "Compiles and installs $(PROGRAM)."
	@echo "To compile / build run: "
	@echo "     \"make build\"" 
	@echo "To Insall run:"
	@echo "     \"make install\""
	@echo ""
	@echo "Once piper is installed, you must schedule it to run. Cron is a"
	@echo "reasonable choice for this, but the specifics of when to run"
	@echo "piper will depend on how frequently you intend to replicate"
	@echo "your zfs datasets, and what your snapshotting schedule is."
	@echo ""
	@echo "Piper also requires a configuration file. By default, piper will"
	@echo "expect the file to be in /usr/local/etc/znappr/piper.json, but"
	@echo "using the -f option, and file can be specified. Use the provided"
	@echo "piper-example.json for an example configuration that can be "
	@echo "customized for your system."
	@echo ""
	@echo "We recommend running piper a few minutes after your snapshots"
	@echo "are scheduled as snapshotting is usually fast, but piper itself"
	@echo "is completely uncoupled with any snapshotting tools you may be"
	@echo "using."
	@echo ""
	@echo "For example, if you run your znappr to make snapshots every hour"
	@echo "on the hour, you might want to run piper 5 or 10 minutes after the"
	@echo "hour. The piper line in your crontab might look like this:"
	@echo "    5 * * * * /usr/local/bin/piper >> /var/log/piper.log 2>&1"
	@echo "    (five minutes after every hour on every hour/day/dow/month)"
	@echo ""
	@echo "If your snapshots are made every 15 mintutes and you want to replicate"
	@echo "as frequently, your crontab line might look like this:"
	@echo "    5,20,35,50 * * * * /usr/local/bin/piper >> /var/log/piper.log 2>&1"
	@echo "    (five/20/35/50 minutes after every hour on every hour/day/dow/month)"
	@echo ""
	@echo "If your only need replication daily, your crontab line might look like this:"
	@echo "    5 0 * * * /usr/local/bin/piper >> /var/log/piper.log 2>&1"
	@echo "    (five minutes after midnight (0) on every day/dow/month)"


build:
	@if [ $$USER = root ];\
	then \
		echo "Do not run make to build $(PROGRAM) as root.\nInstalling with make as root is ok.";\
	else \
		$(CXX);\
	fi

clean: 
	rm -rf target

install:
	cp target/release/$(PROGRAM) /usr/local/bin
	chmod 755 /usr/local/bin/$(PROGRAM)
	mkdir -p /usr/local/etc/znappr
	chmod 755 /usr/local/etc/znappr
	@echo "Installing Piper does not install a configuration file (piper.json)."
	@echo "A configuration file should be installed in /usr/local/etc/znappr/piper.json,"
	@echo "or another location if using piper -f <path to file>."
	@echo "Installing Piper does not run piper. You should add a cron entry to run"
	@echo "Piper as needed."
