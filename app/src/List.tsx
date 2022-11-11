import './App.css';
import Container from 'react-bootstrap/Container';
import Row from 'react-bootstrap/Row';
import Table from 'react-bootstrap/Table';
import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

export default function List() {
  const [profiles, setProfiles] = useState<ProfileSummary[] | undefined>(
    undefined
  );
  useEffect(() => {
    getProfiles().then((p) => setProfiles(p));
  }, []);
  return (
    <main>
      <Container className="text-center py-5">
        <Row>
          <h1>Hotham Profiler</h1>
          <p className="lead">Here you get profiles. I hope you're happy.</p>
        </Row>
      </Container>
      <Container className="py-5">
        <Table striped bordered hover>
          <thead>
            <tr>
              <th>Timestamp</th>
              <th>Change description</th>
              <th>Result</th>
            </tr>
          </thead>
          <tbody>
            {profiles && profiles.map((p) => <ProfileSummaryItem {...p} />)}
          </tbody>
        </Table>
      </Container>
    </main>
  );
}

interface ProfileSummary {
  description: string;
  result: boolean;
  timestamp: number;
  id: number;
}

function ProfileSummaryItem({
  id,
  timestamp,
  result,
  description,
}: ProfileSummary) {
  return (
    <tr className={result ? 'table-success' : 'table-danger'}>
      <td>
        <Link to={`profiles/${timestamp}`}>
          {new Date(timestamp).toLocaleString()}
        </Link>
      </td>
      <td>{description}</td>
      <td>
        <strong>{result ? 'PASS' : 'FAIL'}</strong>
      </td>
    </tr>
  );
}

async function getProfiles(): Promise<ProfileSummary[]> {
  return await (await fetch(`http://localhost:8888/profiles`)).json();
}
