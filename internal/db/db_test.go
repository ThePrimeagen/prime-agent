package db

import (
	"context"
	"database/sql"
	"errors"
	"path/filepath"
	"testing"
)

func TestNewStoreInitializesCounterTableAndSeed(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	count, err := store.IncrementAndGet(context.Background())
	if err != nil {
		t.Fatalf("IncrementAndGet returned error: %v", err)
	}
	if count != 1 {
		t.Fatalf("expected first count to be 1, got %d", count)
	}
}

func TestNewStoreReturnsErrorForInvalidPath(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "missing", "prod.sql")

	_, err := NewStore(dbPath)
	if err == nil {
		t.Fatal("expected NewStore to return an error for invalid path")
	}
}

func TestIncrementAndGetIncrementsByOne(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	first, err := store.IncrementAndGet(context.Background())
	if err != nil {
		t.Fatalf("first IncrementAndGet returned error: %v", err)
	}
	second, err := store.IncrementAndGet(context.Background())
	if err != nil {
		t.Fatalf("second IncrementAndGet returned error: %v", err)
	}
	if second-first != 1 {
		t.Fatalf("expected increment delta to be 1, got %d", second-first)
	}
}

func TestIncrementAndGetReturnsErrorWhenDBUnavailable(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	if err := store.Close(); err != nil {
		t.Fatalf("Close returned error: %v", err)
	}

	_, err = store.IncrementAndGet(context.Background())
	if err == nil {
		t.Fatal("expected IncrementAndGet to return an error on closed DB")
	}
}

func TestSkillCRUDLifecycle(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	skillID, err := store.CreateSkill(ctx, "skill-one", "first prompt")
	if err != nil {
		t.Fatalf("CreateSkill returned error: %v", err)
	}
	if skillID <= 0 {
		t.Fatalf("expected positive skill id, got %d", skillID)
	}

	skills, err := store.ListSkills(ctx)
	if err != nil {
		t.Fatalf("ListSkills returned error: %v", err)
	}
	if len(skills) != 1 {
		t.Fatalf("expected 1 skill, got %d", len(skills))
	}
	if skills[0].Name != "skill-one" || skills[0].Prompt != "first prompt" {
		t.Fatalf("unexpected skill after create: %+v", skills[0])
	}

	skill, err := store.GetSkill(ctx, skillID)
	if err != nil {
		t.Fatalf("GetSkill returned error: %v", err)
	}
	if skill.Name != "skill-one" || skill.Prompt != "first prompt" {
		t.Fatalf("unexpected skill from GetSkill: %+v", skill)
	}

	if err := store.UpdateSkill(ctx, skillID, "skill-one-updated", "updated prompt"); err != nil {
		t.Fatalf("UpdateSkill returned error: %v", err)
	}

	skills, err = store.ListSkills(ctx)
	if err != nil {
		t.Fatalf("ListSkills returned error after update: %v", err)
	}
	if len(skills) != 1 {
		t.Fatalf("expected 1 skill after update, got %d", len(skills))
	}
	if skills[0].Name != "skill-one-updated" || skills[0].Prompt != "updated prompt" {
		t.Fatalf("unexpected skill after update: %+v", skills[0])
	}

	if err := store.DeleteSkill(ctx, skillID); err != nil {
		t.Fatalf("DeleteSkill returned error: %v", err)
	}

	skills, err = store.ListSkills(ctx)
	if err != nil {
		t.Fatalf("ListSkills returned error after delete: %v", err)
	}
	if len(skills) != 0 {
		t.Fatalf("expected 0 skills after delete, got %d", len(skills))
	}
}

func TestSkillMethodsRejectInvalidOrMissingRows(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	if _, err := store.CreateSkill(ctx, "", "prompt"); err == nil {
		t.Fatal("expected CreateSkill to return an error for empty name")
	}
	if _, err := store.CreateSkill(ctx, "name", ""); err == nil {
		t.Fatal("expected CreateSkill to return an error for empty prompt")
	}
	if _, err := store.CreateSkill(ctx, "Bad-Name", "prompt"); err == nil {
		t.Fatal("expected CreateSkill to return an error for uppercase name")
	}
	if _, err := store.CreateSkill(ctx, "bad_name", "prompt"); err == nil {
		t.Fatal("expected CreateSkill to return an error for underscore name")
	}
	if _, err := store.CreateSkill(ctx, "bad name", "prompt"); err == nil {
		t.Fatal("expected CreateSkill to return an error for spaced name")
	}

	if err := store.UpdateSkill(ctx, 999, "name", "prompt"); err == nil {
		t.Fatal("expected UpdateSkill to return an error for missing row")
	} else if err != sql.ErrNoRows {
		t.Fatalf("expected sql.ErrNoRows, got %v", err)
	}

	if err := store.DeleteSkill(ctx, 999); err == nil {
		t.Fatal("expected DeleteSkill to return an error for missing row")
	} else if err != sql.ErrNoRows {
		t.Fatalf("expected sql.ErrNoRows, got %v", err)
	}

	if _, err := store.GetSkill(ctx, 999); err == nil {
		t.Fatal("expected GetSkill to return an error for missing row")
	} else if err != sql.ErrNoRows {
		t.Fatalf("expected sql.ErrNoRows, got %v", err)
	}
}

func TestUpdateSkillRequiresLegacyRenameToKebabCase(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	result, err := store.db.ExecContext(ctx, `INSERT INTO skills (name, prompt) VALUES (?, ?)`, "Legacy_Name", "prompt")
	if err != nil {
		t.Fatalf("failed to seed legacy skill: %v", err)
	}
	skillID, err := result.LastInsertId()
	if err != nil {
		t.Fatalf("failed reading seeded skill id: %v", err)
	}

	if err := store.UpdateSkill(ctx, skillID, "Legacy_Name", "updated prompt"); err == nil {
		t.Fatal("expected UpdateSkill to reject unchanged legacy name")
	}
	if err := store.UpdateSkill(ctx, skillID, "legacy-name", "updated prompt"); err != nil {
		t.Fatalf("expected UpdateSkill to accept kebab rename, got: %v", err)
	}
}

func TestPipelineCRUDLifecycle(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	pipelineID, err := store.CreatePipeline(ctx, "pipeline-one")
	if err != nil {
		t.Fatalf("CreatePipeline returned error: %v", err)
	}
	if pipelineID <= 0 {
		t.Fatalf("expected positive pipeline id, got %d", pipelineID)
	}

	pipelines, err := store.ListPipelines(ctx)
	if err != nil {
		t.Fatalf("ListPipelines returned error: %v", err)
	}
	if len(pipelines) != 1 {
		t.Fatalf("expected 1 pipeline, got %d", len(pipelines))
	}
	if pipelines[0].Name != "pipeline-one" {
		t.Fatalf("unexpected pipeline after create: %+v", pipelines[0])
	}

	pipeline, err := store.GetPipeline(ctx, pipelineID)
	if err != nil {
		t.Fatalf("GetPipeline returned error: %v", err)
	}
	if pipeline.Name != "pipeline-one" {
		t.Fatalf("unexpected pipeline from GetPipeline: %+v", pipeline)
	}
}

func TestPipelineMethodsRejectInvalidOrMissingRows(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	if _, err := store.CreatePipeline(ctx, ""); err == nil {
		t.Fatal("expected CreatePipeline to return an error for empty name")
	}
	if _, err := store.CreatePipeline(ctx, "Bad-Name"); err == nil {
		t.Fatal("expected CreatePipeline to return an error for uppercase name")
	}
	if _, err := store.CreatePipeline(ctx, "bad_name"); err == nil {
		t.Fatal("expected CreatePipeline to return an error for underscore name")
	}
	if _, err := store.CreatePipeline(ctx, "bad name"); err == nil {
		t.Fatal("expected CreatePipeline to return an error for spaced name")
	}

	if _, err := store.GetPipeline(ctx, 999); err == nil {
		t.Fatal("expected GetPipeline to return an error for missing row")
	} else if err != sql.ErrNoRows {
		t.Fatalf("expected sql.ErrNoRows, got %v", err)
	}
}

func TestListPipelineStepsIncludesAttachedSkills(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	pipelineID, err := store.CreatePipeline(ctx, "test-pipeline")
	if err != nil {
		t.Fatalf("CreatePipeline returned error: %v", err)
	}
	stepID, err := store.CreatePipelineStep(ctx, pipelineID, "build", "do build")
	if err != nil {
		t.Fatalf("CreatePipelineStep returned error: %v", err)
	}
	skillLintID, err := store.CreateSkill(ctx, "lint", "run lints")
	if err != nil {
		t.Fatalf("CreateSkill(lint) returned error: %v", err)
	}
	skillTestID, err := store.CreateSkill(ctx, "test", "run tests")
	if err != nil {
		t.Fatalf("CreateSkill(test) returned error: %v", err)
	}
	if err := store.AddPipelineStepSkill(ctx, pipelineID, stepID, skillLintID); err != nil {
		t.Fatalf("AddPipelineStepSkill(lint) returned error: %v", err)
	}
	if err := store.AddPipelineStepSkill(ctx, pipelineID, stepID, skillTestID); err != nil {
		t.Fatalf("AddPipelineStepSkill(test) returned error: %v", err)
	}

	steps, err := store.ListPipelineSteps(ctx, pipelineID)
	if err != nil {
		t.Fatalf("ListPipelineSteps returned error: %v", err)
	}
	if len(steps) != 1 {
		t.Fatalf("expected 1 step, got %d", len(steps))
	}
	if steps[0].SkillCount != 2 {
		t.Fatalf("expected skill count 2, got %d", steps[0].SkillCount)
	}
	if len(steps[0].Skills) != 2 {
		t.Fatalf("expected 2 attached skills, got %d", len(steps[0].Skills))
	}
	if steps[0].Skills[0].Name != "lint" || steps[0].Skills[1].Name != "test" {
		t.Fatalf("expected skills [lint test], got %+v", steps[0].Skills)
	}
}

func TestAddPipelineStepSkillRejectsDuplicateSkill(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "prod.sql")

	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatalf("NewStore returned error: %v", err)
	}
	defer store.Close()

	ctx := context.Background()
	pipelineID, err := store.CreatePipeline(ctx, "test-pipeline")
	if err != nil {
		t.Fatalf("CreatePipeline returned error: %v", err)
	}
	stepID, err := store.CreatePipelineStep(ctx, pipelineID, "build", "do build")
	if err != nil {
		t.Fatalf("CreatePipelineStep returned error: %v", err)
	}
	skillID, err := store.CreateSkill(ctx, "lint", "run lints")
	if err != nil {
		t.Fatalf("CreateSkill returned error: %v", err)
	}
	if err := store.AddPipelineStepSkill(ctx, pipelineID, stepID, skillID); err != nil {
		t.Fatalf("first AddPipelineStepSkill returned error: %v", err)
	}

	err = store.AddPipelineStepSkill(ctx, pipelineID, stepID, skillID)
	if err == nil {
		t.Fatal("expected duplicate AddPipelineStepSkill to return an error")
	}
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("expected ErrConflict, got %v", err)
	}
}
