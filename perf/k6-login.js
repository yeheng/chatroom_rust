import http from 'k6/http';
import { sleep, check } from 'k6';

export let options = {
  vus: 50,
  duration: '30s',
};

export default function () {
  const url = 'http://localhost:8080/api/auth/login';
  const payload = JSON.stringify({ username: 'k6user', password: 'Abc12345' });
  const params = { headers: { 'Content-Type': 'application/json' } };
  const res = http.post(url, payload, params);
  check(res, { 'status is 200': (r) => r.status === 200 });
  sleep(1);
}

