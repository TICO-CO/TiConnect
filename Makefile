PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user lan-test

install-user: target/release/ticonnectd
	install -Dm755 target/release/ticonnectd $(BINDIR)/ticonnectd
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR)
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(AUTOSTART_DIR)/ticonnect.desktop

target/release/ticonnectd:
	cargo build --release --bin ticonnectd

uninstall-user:
	rm -f $(BINDIR)/ticonnectd
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop

lan-test:
	tests/lan/run.sh
