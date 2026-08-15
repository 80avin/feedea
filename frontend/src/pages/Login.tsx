import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { Alert, Button, Card, CardContent, CardHeader, CardTitle, FieldError, Input, Label, TextField } from "@heroui/react";
import { useSession } from "../auth/useSession";

export default function Login() {
  const { session, loading, login } = useSession();
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!loading && session?.authenticated) {
      navigate("/", { replace: true });
    }
  }, [loading, session, navigate]);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError("");
    try {
      await login(password);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-app-bg p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>feedea</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {session?.setup_required && (
            <Alert status="warning">
              <Alert.Title>First-time setup</Alert.Title>
              <Alert.Description>
                No password has been configured yet. The server printed an initial password to its log on startup — use it to log in.
              </Alert.Description>
            </Alert>
          )}
          <form onSubmit={onSubmit} className="flex flex-col gap-4">
            <TextField isInvalid={!!error} fullWidth>
              <Label>Password</Label>
              <Input
                type="password"
                name="password"
                value={password}
                autoFocus
                aria-invalid={!!error}
                onChange={(e) => setPassword(e.target.value)}
              />
              {error && <FieldError>{error}</FieldError>}
            </TextField>
            <Button type="submit" variant="primary" isDisabled={submitting}>
              {submitting ? "Logging in..." : "Log in"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
