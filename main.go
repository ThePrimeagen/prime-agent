package main

import (
	"fmt"
	"log"
	"net/http"
	"os"

	"prime-agent/internal/db"
	"prime-agent/internal/web"
)

const dbPath = "./prod.sql"
const addrEnvKey = "PRIME_AGENT_ADDR"
const dbPathEnvKey = "PRIME_AGENT_DB_PATH"

func serverAddress() string {
	if addr := os.Getenv(addrEnvKey); addr != "" {
		return addr
	}
	return "127.0.0.1:8080"
}

func databasePath() string {
	if path := os.Getenv(dbPathEnvKey); path != "" {
		return path
	}
	return dbPath
}

func run() error {
	store, err := db.NewStore(databasePath())
	if err != nil {
		return fmt.Errorf("database setup failed: %w", err)
	}
	defer func() {
		if err := store.Close(); err != nil {
			log.Printf("database close failed: %v", err)
		}
	}()

	addr := serverAddress()
	log.Printf("listening on http://%s", addr)
	if err := http.ListenAndServe(addr, web.NewMux(store)); err != nil {
		return fmt.Errorf("server failed: %w", err)
	}
	return nil
}

func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}
