package web

import (
	"context"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"prime-agent/internal/db"
)

func TestPipelinePageShowsAttachedStepSkills(t *testing.T) {
	store := newTestStore(t)
	defer store.Close()
	ctx := context.Background()

	pipelineID, err := store.CreatePipeline(ctx, "test-pipeline")
	if err != nil {
		t.Fatalf("CreatePipeline returned error: %v", err)
	}
	stepID, err := store.CreatePipelineStep(ctx, pipelineID, "develop", "do work")
	if err != nil {
		t.Fatalf("CreatePipelineStep returned error: %v", err)
	}
	skillID, err := store.CreateSkill(ctx, "review", "review output")
	if err != nil {
		t.Fatalf("CreateSkill returned error: %v", err)
	}
	if err := store.AddPipelineStepSkill(ctx, pipelineID, stepID, skillID); err != nil {
		t.Fatalf("AddPipelineStepSkill returned error: %v", err)
	}

	req := httptest.NewRequest(http.MethodGet, "/pipelines/"+itoa(pipelineID), nil)
	rec := httptest.NewRecorder()
	NewMux(store).ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}
	body := rec.Body.String()
	if !strings.Contains(body, `data-testid="pipeline-step-skill-summary"`) {
		t.Fatalf("expected skill summary in pipeline nav, body=%q", body)
	}
	if !strings.Contains(body, `data-testid="pipeline-step-attached-skill">review`) {
		t.Fatalf("expected attached skill name to be rendered, body=%q", body)
	}
	if !strings.Contains(body, "/pipelines/"+itoa(pipelineID)+"/steps/"+itoa(stepID)+"/skills/"+itoa(skillID)+"/delete") {
		t.Fatalf("expected attached skill delete action to be rendered, body=%q", body)
	}
}

func TestAddSkillToStepReturnsBadRequestForInvalidSkillID(t *testing.T) {
	store := newTestStore(t)
	defer store.Close()
	ctx := context.Background()

	pipelineID, err := store.CreatePipeline(ctx, "test-pipeline")
	if err != nil {
		t.Fatalf("CreatePipeline returned error: %v", err)
	}
	stepID, err := store.CreatePipelineStep(ctx, pipelineID, "develop", "do work")
	if err != nil {
		t.Fatalf("CreatePipelineStep returned error: %v", err)
	}

	req := httptest.NewRequest(http.MethodPost, "/pipelines/"+itoa(pipelineID)+"/steps/"+itoa(stepID)+"/skills", strings.NewReader("skill_id=abc"))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	rec := httptest.NewRecorder()
	NewMux(store).ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected status 400, got %d", rec.Code)
	}
	if !strings.Contains(rec.Body.String(), "skill_id is required") {
		t.Fatalf("expected validation message, got %q", rec.Body.String())
	}
}

func newTestStore(t *testing.T) *db.Store {
	t.Helper()

	dbPath := filepath.Join(t.TempDir(), "prod.sql")
	store, err := db.NewStore(dbPath)
	if err != nil {
		t.Fatalf("db.NewStore returned error: %v", err)
	}
	return store
}

func itoa(value int64) string {
	return strconv.FormatInt(value, 10)
}
