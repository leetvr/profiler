import Container from 'react-bootstrap/Container';
import Row from 'react-bootstrap/Row';
import Table from 'react-bootstrap/Table';
import { useParams } from 'react-router-dom';
import { useState, useEffect } from 'react';
import _ from 'lodash';

interface MetricSummary {
  name: string;
  value: number;
  lastValue: number;
  averageValue: number;
}
interface ProfileRun {
  timestamp: string;
  description: string;
  metrics: MetricSummary[];
}

export default function Profile() {
  const { profileId } = useParams();
  const [profileRun, setProfileRun] = useState<ProfileRun | undefined>(
    undefined
  );
  useEffect(() => {
    getProfileRun(profileId).then((p) => setProfileRun(p));
  }, [profileId]);

  return (
    <main>
      <Container className="py-5">
        <Row>
          <h1>Profile at {profileRun?.timestamp}</h1>
          <p className="lead">{profileRun?.description}</p>
        </Row>
      </Container>
      <Container>
        <Row>
          {profileRun ? (
            <MetricsTable metrics={profileRun.metrics} />
          ) : (
            <Loading />
          )}
        </Row>
      </Container>
    </main>
  );
}

async function getProfileRun(
  profileId: string | undefined
): Promise<ProfileRun> {
  return await (
    await fetch(`http://localhost:8888/profiles/${profileId}`)
  ).json();
}

function MetricsTable({ metrics }: { metrics: MetricSummary[] }) {
  return (
    <Table striped bordered hover>
      <thead>
        <tr>
          <th>Name</th>
          <th>Value</th>
        </tr>
      </thead>
      <tbody>
        {_.sortBy(metrics, 'name').map((metric) => (
          <MetricRow metric={metric} />
        ))}
      </tbody>
    </Table>
  );
}

function Loading() {
  return <h2>Loading..</h2>;
}

function MetricRow({ metric }: { metric: MetricSummary }) {
  const { name, value, lastValue, averageValue } = metric;
  const tableStyle = getTableStyle(name, value);
  return (
    <tr className={tableStyle}>
      <td>{name}</td>
      <td>{formatValue(name, value)}</td>
    </tr>
  );
}

function getTableStyle(
  key: string,
  value: number
): 'table-default' | 'table-success' | 'table-danger' {
  if (key === 'Total Frame Time' || key === 'GPU Time') {
    return value >= 13 ? 'table-danger' : 'table-success';
  } else {
    return 'table-default';
  }
}

function formatValue(name: string, value: number): string {
  if (name.includes('Bytes')) {
    return `${(value / 1_000_000).toPrecision(4)}MB/s`;
  }
  if (name.includes('Time')) {
    return `${value.toPrecision(4)}ms`;
  }
  if (name.startsWith('%')) {
    return `${value.toPrecision(4)}%`;
  }
  return value.toPrecision(4);
}
