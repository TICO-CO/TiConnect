PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart

.PHONY: install-user uninstall-user

install-user: target/release/ticonnectd
	install -Dm755 target/release/ticonnectd $(BINDIR)/ticonnectd
	install -Dm644 packaging/systemd/ticonnectd.service $(SYSTEMD_USER_DIR)/ticonnectd.service
	install -Dm644 packaging/autostart/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect.desktop

target/release/ticonnectd:
	cargo build --release --bin ticonnectd

uninstall-user:
	rm -f $(BINDIR)/ticonnectd
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
