PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user lan-test

install-user: target/release/ticonnect
	install -Dm755 target/release/ticonnect $(BINDIR)/ticonnect
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR)
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-overlay.service > $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-ui.service > $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-overlay.desktop > $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-ui.desktop > $(AUTOSTART_DIR)/ticonnect-ui.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(SYSTEMD_USER_DIR)/ticonnect-overlay.service $(SYSTEMD_USER_DIR)/ticonnect-ui.service $(AUTOSTART_DIR)/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect-overlay.desktop $(AUTOSTART_DIR)/ticonnect-ui.desktop

target/release/ticonnect:
	cargo build --release --bin ticonnect

uninstall-user:
	rm -f $(BINDIR)/ticonnect
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-ui.desktop

lan-test:
	tests/lan/run.sh
