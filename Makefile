PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user lan-test

install-user: target/release/ticonnectd target/release/ticonnect-ui target/release/ticonnect-ovl target/release/ticonnect-cli
	install -Dm755 target/release/ticonnectd $(BINDIR)/ticonnectd
	install -Dm755 target/release/ticonnect-ui $(BINDIR)/ticonnect-ui
	install -Dm755 target/release/ticonnect-ovl $(BINDIR)/ticonnect-ovl
	install -Dm755 target/release/ticonnect-cli $(BINDIR)/ticonnect-cli
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR)
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-overlay.service > $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-ui.service > $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-overlay.desktop > $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-ui.desktop > $(AUTOSTART_DIR)/ticonnect-ui.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(SYSTEMD_USER_DIR)/ticonnect-overlay.service $(SYSTEMD_USER_DIR)/ticonnect-ui.service $(AUTOSTART_DIR)/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect-overlay.desktop $(AUTOSTART_DIR)/ticonnect-ui.desktop

target/release/ticonnectd:
	cargo build --release --bin ticonnectd

target/release/ticonnect-ui:
	cargo build --release --bin ticonnect-ui

target/release/ticonnect-ovl:
	cargo build --release --bin ticonnect-ovl

target/release/ticonnect-cli:
	cargo build --release --bin ticonnect-cli

uninstall-user:
	rm -f $(BINDIR)/ticonnectd
	rm -f $(BINDIR)/ticonnect-ui
	rm -f $(BINDIR)/ticonnect-ovl
	rm -f $(BINDIR)/ticonnect-cli
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-ui.desktop

lan-test:
	tests/lan/run.sh
