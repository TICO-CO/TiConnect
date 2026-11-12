PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user lan-test

install-user: target/release/ticonnectd target/release/ticonnect-ovl
	install -Dm755 target/release/ticonnectd $(BINDIR)/ticonnectd
	install -Dm755 target/release/ticonnect-ovl $(BINDIR)/ticonnect-ovl
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR)
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-overlay.desktop > $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(AUTOSTART_DIR)/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect-overlay.desktop

target/release/ticonnectd:
	cargo build --release --bin ticonnectd

target/release/ticonnect-ovl:
	cargo build --release --bin ticonnect-ovl

uninstall-user:
	rm -f $(BINDIR)/ticonnectd
	rm -f $(BINDIR)/ticonnect-ovl
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-overlay.desktop

lan-test:
	tests/lan/run.sh
