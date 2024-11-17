// k6 Option Reference: https://k6.io/docs/using-k6/k6-options/reference/

import http from 'k6/http';
import { sleep } from 'k6';

export let options = {
    // https://k6.io/docs/using-k6/k6-options/reference/#stages
    stages: [
        { duration: '100s', target: 5 } 
    ],
    // https://k6.io/docs/using-k6/k6-options/reference/#thresholds
    thresholds: {
        'http_req_duration': ['p(95)<100'],
    },
};

export default function () {
    // post()
    get()
    // sleep(1)
}


function post() {
    const url = 'http://localhost:8083/v1/bid';
    const payload = JSON.stringify({
        trackingId: "test_tracking_id",
        imp: {
            video: {
                w: 640,
                h: 360,
                maxDuration: 40
            }
        },
        signageId: "c59548c3c576228486a1f0037eb16a1b",
        ext: {
            nop: 3,
            views: 2,
            demographic: [0,0,0]
        }
    });

    const params = {
        headers: {
            'Content-Type': 'application/json',
        },
    };
    http.post(url, payload, params);

}

function get() {
    const url = 'http://localhost:8080/plaintext';
    http.get(url);

}
