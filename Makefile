PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APPLICATIONS_DIR ?= $(DATADIR)/applications
ICON_DIR ?= $(DATADIR)/icons/hicolor/scalable/apps
METAINFO_DIR ?= $(DATADIR)/metainfo
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
AUTOSTART_DIR ?= $(HOME)/.config/autostart
XDG_CONFIG_HOME ?= $(HOME)/.config
XDG_DATA_HOME ?= $(HOME)/.local/share
XDG_STATE_HOME ?= $(HOME)/.local/state
DEV_ROOT ?= $(PREFIX)/lib/ticonnect-dev
DEV_BIN ?= $(DEV_ROOT)/bin/ticonnect
DEV_WRAPPER ?= $(BINDIR)/ticonnect-dev
DEV_CONFIG_DIR ?= $(XDG_CONFIG_HOME)/ticonnect-dev
DEV_DATA_DIR ?= $(XDG_DATA_HOME)/ticonnect-dev
DEV_STATE_DIR ?= $(XDG_STATE_HOME)/ticonnect-dev
DEV_RUNTIME_DIR ?= $(if $(XDG_RUNTIME_DIR),$(XDG_RUNTIME_DIR),/tmp/ticonnect-dev-$(shell id -u))/ticonnect-dev
DEV_TRANSPORT_PORT ?= 45881
DEV_DESKTOP ?= $(APPLICATIONS_DIR)/org.ticonnect.TiConnect.Dev.desktop
DEV_ICON ?= $(ICON_DIR)/org.ticonnect.TiConnect.Dev.svg
LAB_RUSTC_WRAPPER ?=
# Keep nix shell RUSTC_WRAPPER (sccache) instead of clearing it.
# Cached objects may hide env/toolchain changes.
LAB_USE_SCCACHE ?= 0
# Pass -v to cargo.
LAB_CARGO_VERBOSE ?= 0

MAKEFLAGS += --no-print-directory

unexport CARGO_BUILD_RUSTFLAGS
export CARGO_ENCODED_RUSTFLAGS =

.PHONY: install-user install-dev uninstall-user uninstall-dev release-bin check-user-surface build-bin lab lab-smoke lan-test web-install web-deps web-dev web-build web-check

install-user: target/release/ticonnect
	install -Dm755 target/release/ticonnect $(BINDIR)/ticonnect
	install -d $(SYSTEMD_USER_DIR) $(AUTOSTART_DIR) $(APPLICATIONS_DIR) $(ICON_DIR) $(METAINFO_DIR)
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.desktop $(APPLICATIONS_DIR)/org.ticonnect.TiConnect.desktop
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.svg $(ICON_DIR)/org.ticonnect.TiConnect.svg
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml $(METAINFO_DIR)/org.ticonnect.TiConnect.metainfo.xml
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnectd.service > $(SYSTEMD_USER_DIR)/ticonnectd.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-overlay.service > $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/systemd/ticonnect-ui.service > $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect.desktop > $(AUTOSTART_DIR)/ticonnect.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-overlay.desktop > $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/autostart/ticonnect-ui.desktop > $(AUTOSTART_DIR)/ticonnect-ui.desktop
	chmod 644 $(SYSTEMD_USER_DIR)/ticonnectd.service $(SYSTEMD_USER_DIR)/ticonnect-overlay.service $(SYSTEMD_USER_DIR)/ticonnect-ui.service $(AUTOSTART_DIR)/ticonnect.desktop $(AUTOSTART_DIR)/ticonnect-overlay.desktop $(AUTOSTART_DIR)/ticonnect-ui.desktop

install-dev: $(if $(TICONNECT_BIN),,release-bin)
	@case "$(DEV_ROOT)" in ""|"/"|"$(HOME)"|"$(HOME)/") \
		echo "refusing unsafe DEV_ROOT: $(DEV_ROOT)" >&2; exit 1;; \
	esac
	@if [ "$(DEV_BIN)" = "$(BINDIR)/ticonnect" ] || [ "$(DEV_WRAPPER)" = "$(BINDIR)/ticonnect" ]; then \
		echo "dev install would overwrite $(BINDIR)/ticonnect" >&2; exit 1; \
	fi
	@if [ "$(DEV_BIN)" = "$(DEV_WRAPPER)" ]; then \
		echo "DEV_BIN and DEV_WRAPPER must differ" >&2; exit 1; \
	fi
	@src="$(if $(TICONNECT_BIN),$(TICONNECT_BIN),target/release/ticonnect)"; \
	test -x "$$src" || { echo "missing executable: $$src" >&2; exit 1; }; \
	install -d -m700 "$(DEV_ROOT)/bin" "$(DEV_CONFIG_DIR)" "$(DEV_DATA_DIR)" "$(DEV_STATE_DIR)" "$(DEV_RUNTIME_DIR)"; \
	install -d -m755 "$(dir $(DEV_WRAPPER))" "$(APPLICATIONS_DIR)" "$(ICON_DIR)"; \
	rm -f "$(DEV_BIN)" "$(DEV_WRAPPER)" "$(DEV_DESKTOP)" "$(DEV_ICON)"; \
	install -Dm755 "$$src" "$(DEV_BIN)"; \
	sed -e 's|@DEV_BIN@|$(DEV_BIN)|g' \
		-e 's|@DEV_CONFIG_DIR@|$(DEV_CONFIG_DIR)|g' \
		-e 's|@DEV_DATA_DIR@|$(DEV_DATA_DIR)|g' \
		-e 's|@DEV_STATE_DIR@|$(DEV_STATE_DIR)|g' \
		-e 's|@DEV_RUNTIME_DIR@|$(DEV_RUNTIME_DIR)|g' \
		-e 's|@DEV_TRANSPORT_PORT@|$(DEV_TRANSPORT_PORT)|g' \
		packaging/dev/ticonnect-dev > "$(DEV_WRAPPER)"; \
	chmod 755 "$(DEV_WRAPPER)"; \
	install -Dm644 packaging/dev/org.ticonnect.TiConnect.Dev.desktop "$(DEV_DESKTOP)"; \
	install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.svg "$(DEV_ICON)"
	@echo "==> install-dev: $(DEV_WRAPPER) -> $(DEV_BIN)"

uninstall-dev:
	@case "$(DEV_ROOT)" in ""|"/"|"$(HOME)"|"$(HOME)/") \
		echo "refusing unsafe DEV_ROOT: $(DEV_ROOT)" >&2; exit 1;; \
	esac
	@case "$(DEV_RUNTIME_DIR)" in ""|"/"|"$(HOME)"|"$(HOME)/") \
		echo "refusing unsafe DEV_RUNTIME_DIR: $(DEV_RUNTIME_DIR)" >&2; exit 1;; \
	esac
	@if [ -x "$(DEV_WRAPPER)" ]; then "$(DEV_WRAPPER)" quit >/dev/null 2>&1 || true; fi
	@if [ -f "$(DEV_RUNTIME_DIR)/bus.pid" ]; then \
		pid="$$(cat "$(DEV_RUNTIME_DIR)/bus.pid" 2>/dev/null || true)"; \
		args="$$(ps -o args= -p "$$pid" 2>/dev/null || true)"; \
		case "$$args" in \
			dbus-daemon*ticonnect-dev*) kill "$$pid" 2>/dev/null || true;; \
		esac; \
	fi
	@rm -f "$(DEV_WRAPPER)" "$(DEV_BIN)" "$(DEV_DESKTOP)" "$(DEV_ICON)"
	@rm -rf "$(DEV_ROOT)" "$(DEV_RUNTIME_DIR)"
	@if [ "$(DEV_PURGE)" = "1" ]; then \
		for path in "$(DEV_CONFIG_DIR)" "$(DEV_DATA_DIR)" "$(DEV_STATE_DIR)"; do \
			case "$$path" in ""|"/"|"$(HOME)"|"$(HOME)/") \
				echo "refusing unsafe purge path: $$path" >&2; exit 1;; \
			esac; \
		done; \
		rm -rf "$(DEV_CONFIG_DIR)" "$(DEV_DATA_DIR)" "$(DEV_STATE_DIR)"; \
	fi

release-bin:
	cargo build --release --bin ticonnect

target/release/ticonnect:
	cargo build --release --bin ticonnect

target/debug/ticonnect:
	CARGO_PROFILE_DEV_DEBUG=0 cargo build --bin ticonnect

build-bin:
	@if [ -n "$(TICONNECT_BIN)" ]; then \
		test -x "$(TICONNECT_BIN)" || { echo "TICONNECT_BIN is not executable: $(TICONNECT_BIN)" >&2; exit 1; }; \
		echo "==> bin: using external binary TICONNECT_BIN=$(TICONNECT_BIN)"; \
		echo "warning: embedded UI freshness depends on the external binary" >&2; \
	else \
		allow="$(TICONNECT_ALLOW_UNDEFINED)"; \
		if [ -z "$$allow" ] && [ -n "$${NIX_LD:-}" ]; then allow=1; fi; \
		wrapper="$(LAB_RUSTC_WRAPPER)"; \
		if [ -z "$$wrapper" ] && [ "$(LAB_USE_SCCACHE)" = "1" ]; then \
			wrapper="$${RUSTC_WRAPPER:-}"; \
			if [ -n "$$wrapper" ]; then \
				echo "warning: LAB_USE_SCCACHE=1; using RUSTC_WRAPPER=$$wrapper (cached objects may hide env/toolchain changes)" >&2; \
			fi; \
		fi; \
		if [ -z "$$wrapper" ] && [ -n "$${RUSTC_WRAPPER:-}" ]; then \
			echo "==> bin: clearing RUSTC_WRAPPER=$$RUSTC_WRAPPER (set LAB_USE_SCCACHE=1 to keep it)"; \
		fi; \
		echo "==> bin: rustc wrapper='$$wrapper', debug info off"; \
		vflag=""; \
		case "$(LAB_CARGO_VERBOSE)" in 1|true|yes) vflag="-v" ;; esac; \
		if [ "$$allow" = "1" ]; then \
			echo "==> bin: cargo rustc (NIX_LD set: allow undefined shlibs)"; \
			RUSTC_WRAPPER="$$wrapper" CARGO_PROFILE_DEV_DEBUG=0 cargo rustc $$vflag -p ticonnect --bin ticonnect -- -C link-arg=-Wl,--allow-shlib-undefined; \
		else \
			echo "==> bin: cargo build --bin ticonnect"; \
			RUSTC_WRAPPER="$$wrapper" CARGO_PROFILE_DEV_DEBUG=0 cargo build $$vflag --bin ticonnect; \
		fi; \
	fi
	@echo "==> bin: done"

lab:
	@if [ "$${LAB_HOT_RELOAD:-1}" = "1" ]; then \
		echo "==> lab: hot reload mode (web-deps, then build-bin)"; \
		$(MAKE) web-deps; \
		echo "==> lab: building ticonnect binary"; \
		$(MAKE) build-bin; \
	else \
		echo "==> lab: embedded mode (web-build, then build-bin)"; \
		$(MAKE) web-build; \
		echo "==> lab: building ticonnect binary"; \
		$(MAKE) build-bin; \
	fi
	@echo "==> lab: starting tests/local/lab.sh"
	TICONNECT_BIN="$(TICONNECT_BIN)" tests/local/lab.sh

lab-smoke:
	@echo "==> lab-smoke: embedded mode (web-build, then build-bin)"
	$(MAKE) web-build
	$(MAKE) build-bin
	@echo "==> lab-smoke: running tests/local/lab.sh --smoke"
	TICONNECT_BIN="$(TICONNECT_BIN)" tests/local/lab.sh --smoke

uninstall-user:
	rm -f $(BINDIR)/ticonnect
	rm -f $(APPLICATIONS_DIR)/org.ticonnect.TiConnect.desktop
	rm -f $(ICON_DIR)/org.ticonnect.TiConnect.svg
	rm -f $(METAINFO_DIR)/org.ticonnect.TiConnect.metainfo.xml
	rm -f $(SYSTEMD_USER_DIR)/ticonnectd.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-overlay.service
	rm -f $(SYSTEMD_USER_DIR)/ticonnect-ui.service
	rm -f $(AUTOSTART_DIR)/ticonnect.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-overlay.desktop
	rm -f $(AUTOSTART_DIR)/ticonnect-ui.desktop

check-user-surface: target/release/ticonnect
	@if target/release/ticonnect --help | grep -E '^[[:space:]]*(daemon|overlay|cli)\b'; then \
		echo "internal command exposed in user help" >&2; \
		exit 1; \
	fi

lan-test:
	tests/lan/run.sh

WEB_DIR ?= crates/ui/web
UI_ASSETS ?= crates/ui/assets

web-install:
	@echo "==> web: bun install --frozen-lockfile"
	cd $(WEB_DIR) && bun install --frozen-lockfile

web-deps:
	@if [ ! -x "$(WEB_DIR)/node_modules/.bin/vite" ]; then \
		echo "==> web: installing deps (node_modules/.bin/vite missing)"; \
		$(MAKE) web-install; \
	elif [ "$(WEB_DIR)/bun.lock" -nt "$(WEB_DIR)/node_modules" ]; then \
		echo "==> web: installing deps (bun.lock newer than node_modules)"; \
		$(MAKE) web-install; \
	else \
		echo "==> web: deps up to date"; \
	fi

web-dev: web-deps
	cd $(WEB_DIR) && bun run dev

web-build: web-deps
	@echo "==> web: vite build"
	cd $(WEB_DIR) && bun run build
	@if command -v rsync >/dev/null 2>&1; then \
		echo "==> web: syncing dist -> $(UI_ASSETS) (rsync)"; \
		rsync -rlpgoD --checksum --delete $(WEB_DIR)/dist/ $(UI_ASSETS)/; \
	else \
		echo "==> web: syncing dist -> $(UI_ASSETS) (cp)"; \
		rm -rf $(UI_ASSETS); \
		mkdir -p $(UI_ASSETS); \
		cp -R $(WEB_DIR)/dist/. $(UI_ASSETS)/; \
	fi

web-check: web-deps
	cd $(WEB_DIR) && bun run format:check && bun run lint && bun run check && bun run test
