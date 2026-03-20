package main

import (
	"net"
	"path/filepath"
	"strings"
	"testing"
)

func TestServerAddressIsLoopback(t *testing.T) {
	t.Setenv(addrEnvKey, "")
	if got := serverAddress(); got != "127.0.0.1:8080" {
		t.Fatalf("expected loopback address 127.0.0.1:8080, got %q", got)
	}
}

func TestServerAddressUsesEnvironmentOverride(t *testing.T) {
	t.Setenv(addrEnvKey, "127.0.0.1:18080")
	if got := serverAddress(); got != "127.0.0.1:18080" {
		t.Fatalf("expected env address 127.0.0.1:18080, got %q", got)
	}
}

func TestDatabasePathUsesEnvironmentOverride(t *testing.T) {
	t.Setenv(dbPathEnvKey, "/tmp/prime-agent-test.db")
	if got := databasePath(); got != "/tmp/prime-agent-test.db" {
		t.Fatalf("expected env db path override, got %q", got)
	}
}

func TestRunReturnsErrorWhenDatabasePathIsInvalid(t *testing.T) {
	t.Setenv(addrEnvKey, "127.0.0.1:0")
	t.Setenv(dbPathEnvKey, filepath.Join(t.TempDir(), "missing", "prod.sql"))

	err := run()
	if err == nil {
		t.Fatal("expected run to fail for invalid database path")
	}
	if !strings.Contains(err.Error(), "database setup failed") {
		t.Fatalf("expected database setup failure, got %v", err)
	}
}

func TestRunReturnsErrorWhenAddressIsAlreadyInUse(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to reserve address: %v", err)
	}
	defer listener.Close()

	t.Setenv(addrEnvKey, listener.Addr().String())
	t.Setenv(dbPathEnvKey, filepath.Join(t.TempDir(), "prod.sql"))

	err = run()
	if err == nil {
		t.Fatal("expected run to fail when address is already in use")
	}
	if !strings.Contains(err.Error(), "server failed") {
		t.Fatalf("expected server failure, got %v", err)
	}
}
