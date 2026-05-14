import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { enterMainApp, saveConfig } from "../api/commands";
import { StepEmail } from "../components/Setup/StepEmail";
import { StepToken } from "../components/Setup/StepToken";
import { StepUrl } from "../components/Setup/StepUrl";
import { Wizard } from "../components/Setup/Wizard";

/**
 * 3-step setup wizard. Owns the form state (`url`, `email`, `token`) and the
 * current step index; the step components are otherwise self-contained
 * (validation + UI feedback).
 *
 * On finish we:
 * 1. Persist the config + token via `save_config` (writes config.toml +
 *    keychain, rebuilds the in-memory JiraClient).
 * 2. Ask the backend to navigate the main window to the main app view.
 * 3. Navigate locally so the user immediately sees `/`.
 */
export default function Setup() {
  const navigate = useNavigate();

  const [step, setStep] = useState(0);
  const [url, setUrl] = useState("");
  const [email, setEmail] = useState("");
  const [token, setToken] = useState("");
  const [submitError, setSubmitError] = useState<string | null>(null);

  async function handleFinish() {
    setSubmitError(null);
    try {
      await saveConfig({ base_url: url, email }, token);
      await enterMainApp();
      navigate("/", { replace: true });
    } catch (e) {
      const msg =
        typeof e === "string" ? e : e instanceof Error ? e.message : "save failed";
      setSubmitError(msg);
      throw e;
    }
  }

  return (
    <Wizard step={step}>
      {step === 0 && (
        <StepUrl value={url} onChange={setUrl} onNext={() => setStep(1)} />
      )}
      {step === 1 && (
        <StepEmail
          value={email}
          onChange={setEmail}
          onNext={() => setStep(2)}
          onBack={() => setStep(0)}
        />
      )}
      {step === 2 && (
        <StepToken
          value={token}
          onChange={setToken}
          onFinish={handleFinish}
          onBack={() => setStep(1)}
          baseUrl={url}
          email={email}
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
    </Wizard>
  );
}
