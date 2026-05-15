import { useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  addConnection,
  enterMainApp,
  listFreeloProjects,
  saveConfig,
  setFreeloSelectedProjects,
  syncFreeloNow,
} from "../api/commands";
import type { ProviderKind, ProviderUser } from "../api/types";
import { StepEmail } from "../components/Setup/StepEmail";
import { StepFreeloCreds } from "../components/Setup/StepFreeloCreds";
import { StepFreeloProjects } from "../components/Setup/StepFreeloProjects";
import { StepProvider } from "../components/Setup/StepProvider";
import { StepToken } from "../components/Setup/StepToken";
import { StepUrl } from "../components/Setup/StepUrl";
import {
  FREELO_SETUP_STEPS,
  JIRA_SETUP_STEPS,
  Wizard,
} from "../components/Setup/Wizard";

/**
 * Multi-provider setup wizard (Phase 18E).
 *
 * Step 1 is always the provider picker. From there each provider runs its own
 * flow:
 *   - Jira: URL → email → token (test+finish). 4 steps total counting the
 *     picker.
 *   - Freelo: credentials (email + key + test) → project picker. 3 steps.
 */
export default function Setup() {
  const navigate = useNavigate();

  const [step, setStep] = useState(0);
  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);

  // Jira state.
  const [url, setUrl] = useState("");
  const [email, setEmail] = useState("");
  const [token, setToken] = useState("");

  // Freelo state.
  const [freeloEmail, setFreeloEmail] = useState("");
  const [freeloApiKey, setFreeloApiKey] = useState("");
  const [freeloBaseUrl, setFreeloBaseUrl] = useState(
    "https://api.freelo.io/v1",
  );
  const [freeloUser, setFreeloUser] = useState<ProviderUser | null>(null);
  const [freeloConnectionId, setFreeloConnectionId] = useState<number | null>(
    null,
  );

  async function handleJiraFinish() {
    setSubmitError(null);
    try {
      await saveConfig({ base_url: url, email }, token);
      await enterMainApp();
      navigate("/", { replace: true });
    } catch (e) {
      const msg =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "save failed";
      setSubmitError(msg);
      throw e;
    }
  }

  /**
   * After the user successfully tests Freelo credentials, we persist the
   * connection (so the project picker can call `list_freelo_projects` against
   * it) and advance to the project step.
   */
  async function handleFreeloTested(user: ProviderUser) {
    setSubmitError(null);
    setFreeloUser(user);
    try {
      const conn = await addConnection({
        provider: "freelo",
        name: `Freelo · ${user.displayName}`,
        config: {
          base_url: freeloBaseUrl,
          email: freeloEmail,
          selected_project_ids: [],
          sync_user_id: Number(user.accountId),
        },
        token: freeloApiKey,
      });
      setFreeloConnectionId(conn.id);
      setStep(2);
    } catch (e) {
      const msg =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "save failed";
      setSubmitError(msg);
    }
  }

  async function handleFreeloFinish(projectIds: number[]) {
    setSubmitError(null);
    if (freeloConnectionId == null) {
      setSubmitError("Vnitřní chyba: chybí id připojení");
      return;
    }
    try {
      await setFreeloSelectedProjects(freeloConnectionId, projectIds);
      // Best-effort: kick off an initial sync. We ignore errors here so the
      // user can proceed to the main app even if the first sync is slow.
      try {
        await syncFreeloNow(freeloConnectionId);
      } catch {
        /* ignore */
      }
      await enterMainApp();
      navigate("/", { replace: true });
    } catch (e) {
      const msg =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "save failed";
      setSubmitError(msg);
    }
  }

  // Pick the step list + title based on the chosen provider.
  const steps =
    provider === "freelo" ? FREELO_SETUP_STEPS : JIRA_SETUP_STEPS;
  const title =
    provider === "freelo"
      ? "Připojit Freelo"
      : provider === "jira"
        ? "Připojit Jira"
        : "Připojit účet";

  return (
    <Wizard step={step} steps={steps} title={title}>
      {step === 0 && (
        <StepProvider
          value={provider}
          onChange={(p) => setProvider(p)}
          onNext={() => setStep(1)}
        />
      )}

      {/* Jira flow (steps 1..3) */}
      {provider === "jira" && step === 1 && (
        <StepUrl value={url} onChange={setUrl} onNext={() => setStep(2)} />
      )}
      {provider === "jira" && step === 2 && (
        <StepEmail
          value={email}
          onChange={setEmail}
          onNext={() => setStep(3)}
          onBack={() => setStep(1)}
        />
      )}
      {provider === "jira" && step === 3 && (
        <StepToken
          value={token}
          onChange={setToken}
          onFinish={handleJiraFinish}
          onBack={() => setStep(2)}
          baseUrl={url}
          email={email}
        />
      )}

      {/* Freelo flow (steps 1..2) */}
      {provider === "freelo" && step === 1 && (
        <StepFreeloCreds
          email={freeloEmail}
          apiKey={freeloApiKey}
          baseUrl={freeloBaseUrl}
          onChangeEmail={setFreeloEmail}
          onChangeApiKey={setFreeloApiKey}
          onChangeBaseUrl={setFreeloBaseUrl}
          onTested={handleFreeloTested}
          onBack={() => setStep(0)}
        />
      )}
      {provider === "freelo" && step === 2 && freeloConnectionId != null && (
        <StepFreeloProjects
          fetchProjects={() => listFreeloProjects(freeloConnectionId)}
          onFinish={handleFreeloFinish}
          onBack={() => setStep(1)}
        />
      )}

      {submitError && (
        <p
          className="mt-4 text-xs text-[var(--danger)]"
          role="alert"
          data-testid="setup-submit-error"
        >
          {submitError}
        </p>
      )}

      {/* For tests / accessibility: mark that we're showing the Freelo user so
          tests can pick it up without coupling to step internals. */}
      {freeloUser && step >= 2 && (
        <span data-testid="freelo-tested-name" className="hidden">
          {freeloUser.displayName}
        </span>
      )}
    </Wizard>
  );
}
